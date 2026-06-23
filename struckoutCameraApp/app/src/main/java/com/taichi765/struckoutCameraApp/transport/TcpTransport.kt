package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext
import kotlinx.io.IOException
import timber.log.Timber
import java.io.InputStream
import java.io.OutputStream
import java.net.Socket


class TcpTransport {

    private val internalState =
        MutableStateFlow<InternalConnectionState>(InternalConnectionState.Disconnected)

    val state = internalState.map {
        when (it) {
            is InternalConnectionState.Connected -> ConnectionState.Connected(it.cameraID)
            is InternalConnectionState.Disconnected -> ConnectionState.Disconnected
        }
    }

    suspend fun connect(): Boolean {
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
                        internalState.value = InternalConnectionState.Connected(
                            cameraId, socket, inputStream, socket.getOutputStream()
                        )
                        Timber.tag(TAG).i("successfully initialized connection states")
                    }

                    Struckout.TcpServerPacket.DataCase.DATA_NOT_SET -> Timber.tag(TAG)
                        .w("received invalid TCP packet from server")
                }
                return@withContext true
            } catch (e: IOException) {
                Timber.tag(TAG).w("failed to initialize connection states: $e")
                return@withContext false
            }
        }
    }

    suspend fun close() {
        val state = internalState.value
        if (state !is InternalConnectionState.Connected) {
            Timber.tag(TAG).w("close() is called when TCP is not connected")
            return
        }
        withContext(Dispatchers.IO) {
            Timber.tag(TAG).i("closing TCP socket")
            state.socket.close()
        }
    }

    suspend fun sendPacket(packet: Struckout.TcpClientPacket) {
        val curState = internalState.value
        check(curState is InternalConnectionState.Connected) {
            "TCP connection must be established before sending packet"
        }
        check(packet.dataCase != Struckout.TcpClientPacket.DataCase.DATA_NOT_SET) {
            "Packet data must be set"
        }

        withContext(Dispatchers.IO) {
            Timber.tag(TAG).i("sending TCP packet")
            try {
                writePacket(curState.outputStream, packet)
            } catch (e: IOException) {
                // TODO: exceptionの理由を画面に表示したほうがいいのだろうか
                Timber.tag(TAG).w("it seems that TCP connection is unexpectedly closed: $e")
                internalState.value = InternalConnectionState.Disconnected
            }
        }
    }

    private sealed interface InternalConnectionState {
        data class Connected(
            val cameraID: UInt,
            val socket: Socket,
            val inputStream: InputStream,
            val outputStream: OutputStream
        ) : InternalConnectionState

        object Disconnected : InternalConnectionState
    }

    companion object {
        const val TAG = "TcpTransport"

        const val TCP_REMOTE_ADDRESS = "192.168.10.110"

        const val TCP_REMOTE_PORT = 6060
    }
}

sealed interface ConnectionState {
    data class Connected(val cameraID: UInt) : ConnectionState

    object Disconnected : ConnectionState
}