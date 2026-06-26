package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.di.ApplicationScope
import com.taichi765.struckoutCameraApp.proto.Struckout
import com.taichi765.struckoutCameraApp.proto.TcpClientPacketKt
import com.taichi765.struckoutCameraApp.proto.tcpClientPacket
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.io.IOException
import timber.log.Timber
import java.io.OutputStream
import java.net.Socket
import javax.inject.Inject

class TcpSessionRepository @Inject constructor(
    @ApplicationScope private val scope: CoroutineScope,
    cameraLocationDataSource: CameraLocationDataSource
) : SessionRepository {
    private val _connState =
        MutableStateFlow<SessionState>(SessionState.DisConnected)

    override val state = _connState.asStateFlow()

    private val outChannel =
        Channel<OutputAction>(capacity = 4, onBufferOverflow = BufferOverflow.DROP_LATEST)

    init {
        cameraLocationDataSource.cameraLocation.onEach { location ->
            val curState = _connState.value
            if (curState !is SessionState.Connected) {
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
            } catch (e: IOException) {
                Timber.tag(TAG).w("failed to connect to TCP server: $e")
                return@withContext false
            }

            Timber.tag(TAG).i("initializing states via TCP")
            try {
                val inputStream = socket.getInputStream()
                val packet = readPacket(inputStream, Struckout.TcpServerPacket::parseFrom)
                when (packet.dataCase) {
                    Struckout.TcpServerPacket.DataCase.CAMERA_ID -> {
                        val cameraId = packet.cameraId.toUInt()
                        _connState.value = SessionState.Connected(
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

    /**
     * [outChannel]から[OutputAction]を読んでTCPサーバーに色々送る
     */
    private suspend fun outputActor(output: OutputStream) {
        for (action in outChannel) {
            val curState = _connState.value
            check(curState is SessionState.Connected)
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

    companion object {
        const val TAG = "TcpTransport"

        const val TCP_REMOTE_ADDRESS = "192.168.10.110"

        const val TCP_REMOTE_PORT = 6060
        const val DUMMY_CAMERA_ID = 99u
    }
}
