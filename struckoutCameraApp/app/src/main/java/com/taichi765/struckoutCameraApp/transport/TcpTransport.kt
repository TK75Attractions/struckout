package com.taichi765.struckoutCameraApp.transport

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext
import kotlinx.io.IOException
import struckout.v1.Struckout
import java.io.InputStream
import java.io.OutputStream
import java.net.Socket


class TcpTransport : TcpTransportRepository {

    private val internalState =
        MutableStateFlow<InternalConnectionState>(InternalConnectionState.Disconnected)

    override val state = internalState.map {
        when (it) {
            is InternalConnectionState.Connected -> ConnectionState.Connected(it.cameraID)
            is InternalConnectionState.Disconnected -> ConnectionState.Disconnected
        }
    }

    override suspend fun connect(): Boolean {
        return withContext(Dispatchers.IO) {
            val socket = try {
                Log.i(TAG, "connecting to ball_watcher")
                val ret = Socket(TCP_REMOTE_ADDRESS, TCP_REMOTE_PORT)
                Log.i(TAG, "TCP connection has been established successfully")
                ret
            } catch (e: IOException) {
                Log.w(TAG, "failed to connect to TCP server: $e")
                return@withContext false
            }

            try {
                Log.i(TAG, "initializing states via TCP")

                val inputStream = socket.getInputStream()
                val packet = readPacket(inputStream, Struckout.TcpServerPacket::parseFrom)
                when (packet.dataCase) {
                    Struckout.TcpServerPacket.DataCase.CAMERA_ID -> {
                        val cameraId = packet.cameraId.toUInt()
                        internalState.value = InternalConnectionState.Connected(
                            cameraId, socket, inputStream, socket.getOutputStream()
                        )
                        Log.i(TAG, "successfully initialized connection states")
                    }

                    Struckout.TcpServerPacket.DataCase.DATA_NOT_SET -> Log.w(
                        TAG,
                        "received invalid TCP packet from server"
                    )
                }
                return@withContext true
            } catch (e: IOException) {
                Log.w(TAG, "failed to initialize connection states: $e")
                return@withContext false
            }
        }
    }

    override suspend fun close() {
        val state = internalState.value
        if (state !is InternalConnectionState.Connected) {
            Log.w(TAG, "close() is called when TCP is not connected")
            return
        }
        withContext(Dispatchers.IO) {
            Log.i(TAG, "closing TCP socket")
            state.socket.close()
        }
    }

    override suspend fun sendPacket(packet: Struckout.TcpClientPacket) {
        val curState = internalState.value
        check(curState is InternalConnectionState.Connected) {
            "TCP connection must be established before sending packet"
        }
        withContext(Dispatchers.IO) {
            Log.i(TAG, "sending TCP packet")
            writePacket(curState.outputStream, packet)
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
