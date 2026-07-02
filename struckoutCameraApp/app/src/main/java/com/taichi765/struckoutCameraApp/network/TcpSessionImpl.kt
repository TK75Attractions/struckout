package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.proto.Struckout
import com.taichi765.struckoutCameraApp.proto.TcpClientPacketKt
import com.taichi765.struckoutCameraApp.proto.tcpClientPacket
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import timber.log.Timber
import java.io.Closeable
import java.io.IOException
import java.io.OutputStream
import java.net.Socket
import javax.inject.Inject

/**
 * [NetworkManager]がインスタンスのライフサイクルを管理しているので他のところで
 * 直接使うべからず
 */
class TcpSessionImpl(
    cameraLocationDataSource: CameraLocationDataSource
) : TcpSession, SessionStateProvider, Closeable {
    /**
     * [outputActor]や[CameraLocationDataSource]のライフサイクルをApplicationやViewModelではなく
     * 自前で管理したいため
     */
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val _connState =
        MutableStateFlow<InternalSessionState>(InternalSessionState.DisConnected)
    override val state = _connState.map { state ->
        when (state) {
            is InternalSessionState.Connected -> SessionState.Connected(state.cameraID)
            is InternalSessionState.DisConnected -> SessionState.DisConnected
        }
    }.stateIn(
        scope = scope,
        started = SharingStarted.Eagerly,
        initialValue = SessionState.DisConnected
    )

    private val outChannel =
        Channel<OutputAction>(capacity = 4, onBufferOverflow = BufferOverflow.DROP_LATEST)


    init {
        cameraLocationDataSource.cameraLocation.onEach { location ->
            val curState = _connState.value
            if (curState !is InternalSessionState.Connected) {
                Timber.tag(TAG)
                    .w("camera location was updated but cannot sync change to TCP server: TCP is not connected")
                return@onEach
            }
            outChannel.send(OutputAction.UpdateCameraLocation(location))
        }.launchIn(scope)
    }

    override suspend fun connect(): Boolean {
        return withContext(Dispatchers.IO) {
            Timber.tag(TAG).i("connecting to ball_watcher")
            val socket = try {
                val ret = Socket(TCP_REMOTE_ADDRESS, TCP_REMOTE_PORT)
                Timber.tag(TAG).i("TCP connection has been established successfully")
                ret
            } catch (e: Exception) {
                when (e) {
                    is TimeoutCancellationException,
                    is IOException -> {
                        Timber.tag(TAG).w("failed to connect to TCP server: $e")
                        return@withContext false
                    }

                    else -> throw e
                }
            }

            Timber.tag(TAG).i("initializing states via TCP")
            try {
                val inputStream = socket.getInputStream()
                val packet = readPacket(inputStream, Struckout.TcpServerPacket::parseFrom)
                when (packet.dataCase) {
                    Struckout.TcpServerPacket.DataCase.CAMERA_ID -> {
                        val cameraId = packet.cameraId.toUInt()
                        _connState.value = InternalSessionState.Connected(
                            socket,
                            cameraId,
                        )
                        Timber.tag(TAG).i("successfully initialized connection states")
                    }

                    Struckout.TcpServerPacket.DataCase.DATA_NOT_SET -> Timber.tag(TAG)
                        .w("received invalid TCP packet from server")
                }
            } catch (e: IOException) {
                Timber.tag(TAG).w("failed to initialize connection states: $e")
                return@withContext false
            }

            scope.launch {
                outputActor(socket.getOutputStream())
            }
            return@withContext true
        }
    }

    override fun close() {
        val curState = _connState.value
        if (curState is InternalSessionState.Connected) {
            curState.socket.close()
        } else {
            Timber.tag(TAG).d("TcpSessionRepository.close() is called, but TCP is not connected")
        }
        scope.cancel()
    }

    /**
     * [outChannel]から[OutputAction]を読んでTCPサーバーに色々送る
     */
    private suspend fun outputActor(output: OutputStream) {
        for (action in outChannel) {
            val curState = _connState.value
            check(curState is InternalSessionState.Connected)
            when (action) {
                is OutputAction.UpdateCameraLocation -> writePacket(output, tcpClientPacket {
                    this.cameraLoc = TcpClientPacketKt.updateCameraLocation {
                        cameraId = curState.cameraID.toInt()
                        cameraLocation = action.location
                    }
                })
            }
        }
    }

    /**
     * Tells [outChannel] which actions to run.
     */
    private sealed interface OutputAction {
        data class UpdateCameraLocation(val location: Struckout.CameraLocation) : OutputAction
    }

    private sealed interface InternalSessionState {
        data class Connected(
            val socket: Socket,
            val cameraID: UInt,
        ) : InternalSessionState

        object DisConnected : InternalSessionState
    }

    class Factory @Inject constructor(private val cameraLocationDataSource: CameraLocationDataSource) :
        TcpSession.Factory {
        override fun create(): TcpSession {
            return TcpSessionImpl(cameraLocationDataSource)
        }
    }

    companion object {
        const val TAG = "TcpSessionImpl"

        const val TCP_REMOTE_ADDRESS = "127.0.0.1"

        const val TCP_REMOTE_PORT = 6060
    }
}
