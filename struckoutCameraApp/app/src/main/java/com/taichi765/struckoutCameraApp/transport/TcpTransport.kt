package com.taichi765.struckoutCameraApp.transport

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.io.IOException
import struckout.Struckout
import java.net.Socket

/**
 * TODO: set appropriate address (current one is dummy)
 */
const val TCP_REMOTE_ADDRESS = "192.168.0.0"

const val TCP_REMOTE_PORT = 5050
const val TCP_LOCAL_PORT = 8833

class TcpTransport {
    private var socket: Socket? = null


    suspend fun connect(): Boolean {
        return withContext(Dispatchers.IO) {
            try {
                socket = Socket(TCP_REMOTE_ADDRESS, TCP_REMOTE_PORT, null, TCP_LOCAL_PORT)
                //socket!!.getInputStream()
                return@withContext true
            } catch (e: IOException) {
                Log.w(TAG, "failed to connect to TCP server: $e")
                return@withContext false
            }
        }
    }

    suspend fun sendPacket(packet: Struckout.TcpClientPacket) {
        withContext(Dispatchers.IO) {
            socket?.getOutputStream().use {
                packet.writeTo(it)
            }
        }
    }

    companion object {
        const val TAG = "TcpTransport"
    }
}