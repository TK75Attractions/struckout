package com.taichi765.struckoutCameraApp.transport

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.io.IOException
import struckout.Struckout
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress

/**
 * TODO: set proper address (current one is dummy)
 */
val UDP_REMOTE_ADDRESS: InetAddress = InetAddress.getByName("192.168.0.0")

/**
 * Send packet to port 5050
 */
const val UDP_REMOTE_PORT = 5050

/**
 * Receive packet at port 8822
 */
const val UDP_LOCAL_PORT = 8822

class UdpTransport {
    private var socket: DatagramSocket? = null

    /**
     * Creates new UDP socket and bind it to the port for receiving data from server.
     * @return
     * Returns whether binding is succeeded or not.
     */
    suspend fun bind(): Boolean {
        return withContext(Dispatchers.IO) {
            try {
                socket = DatagramSocket(UDP_LOCAL_PORT)
                socket!!.connect(UDP_REMOTE_ADDRESS, UDP_REMOTE_PORT)
                return@withContext true
            } catch (e: IOException) {
                Log.w(TAG, "failed to bind port $UDP_LOCAL_PORT: $e")
                return@withContext false
            }
        }
    }

    suspend fun sendPacket(packet: Struckout.UdpPacket) {
        val bytes = packet.toByteArray()
        val p = DatagramPacket(bytes, bytes.size)

        withContext(Dispatchers.IO) {
            socket?.send(p)
        }
    }

    companion object {
        const val TAG = "UdpTransport"
    }
}