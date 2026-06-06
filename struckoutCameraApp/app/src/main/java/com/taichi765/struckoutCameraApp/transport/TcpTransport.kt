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

    private val _isConnected = MutableStateFlow(false)
    override val isConnected = _isConnected.asStateFlow()


    override suspend fun connect(): Boolean {
        return withContext(Dispatchers.IO) {
            try {
                Log.i(TAG, "connecting to ball_watcher")
                socket = Socket(TCP_REMOTE_ADDRESS, TCP_REMOTE_PORT)
                _isConnected.value = true
                //socket!!.getInputStream()
                return@withContext true
            } catch (e: IOException) {
                Log.w(TAG, "failed to connect to TCP server: $e")
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