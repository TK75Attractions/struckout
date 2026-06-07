package com.taichi765.struckoutCameraApp.transport

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import kotlinx.io.IOException
import struckout.v1.Struckout
import java.net.Socket


class TcpTransport : TcpTransportRepository {
    private var socket: Socket? = null

    private val _state = MutableStateFlow<ConnectionState>(ConnectionState.Disconnected)
    override val state = _state.asStateFlow()


    override suspend fun connect(): Boolean {
        return withContext(Dispatchers.IO) {
            try {
                Log.i(TAG, "connecting to ball_watcher")
                socket = Socket(TCP_REMOTE_ADDRESS, TCP_REMOTE_PORT)
                Log.i(TAG, "TCP connection has been established successfully")
            } catch (e: IOException) {
                Log.w(TAG, "failed to connect to TCP server: $e")
                return@withContext false
            }

            try {
                Log.i(TAG, "initializing states via TCP")
                val packet = socket!!.getInputStream().use {
                    readPacket(it, Struckout.TcpServerPacket::parseFrom)
                }
                when (packet.dataCase) {
                    Struckout.TcpServerPacket.DataCase.CAMERA_ID -> {
                        val cameraId = packet.cameraId.toUInt()
                        _state.value = ConnectionState.Connected(cameraId)
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
        withContext(Dispatchers.IO) {
            socket?.close()
        }
    }

    override suspend fun sendPacket(packet: Struckout.TcpClientPacket) {
        withContext(Dispatchers.IO) {
            val socket: Socket = socket
                ?: throw IllegalStateException("$TAG.sendPacket() is called before TCP connection is established")

            socket.getOutputStream().use {
                assert(it != null)
                packet.writeTo(it)
            }
        }
    }

    companion object {
        const val TAG = "TcpTransport"

        /**
         * TODO: set appropriate address (current one is dummy)
         */
        const val TCP_REMOTE_ADDRESS = "192.168.10.110"

        const val TCP_REMOTE_PORT = 6060
    }
}
