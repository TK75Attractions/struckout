package com.taichi765.struckoutCameraApp.transport

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext
import kotlinx.io.IOException
import struckout.v1.Struckout
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress


class UdpTransport : UdpTransportRepository {
    private var socket = MutableStateFlow<DatagramSocket?>(null)

    /**
     * Whether the socket is bound to a port or not.
     */
    override val isBound = socket.map {
        it != null
    }

    /**
     * Creates new UDP socket and bind it to the port for receiving data from server.
     * @return
     * Returns whether binding is succeeded or not.
     */
    override suspend fun bind(): Boolean {
        return withContext(Dispatchers.IO) {
            try {
                val newSocket = DatagramSocket(UDP_LOCAL_PORT)
                socket.value = newSocket
                newSocket.connect(UDP_REMOTE_ADDRESS, UDP_REMOTE_PORT)
                return@withContext true
            } catch (e: IOException) {
                Log.w(TAG, "failed to bind port $UDP_LOCAL_PORT: $e")
                return@withContext false
            }
        }
    }

    override suspend fun sendPacket(packet: Struckout.UdpPacket) {
        val curSocket = socket.value
        check(curSocket != null) {
            "UDP port must be bound to port before sending packet"
        }

        val bytes = packet.toByteArray()
        val p = DatagramPacket(bytes, bytes.size)

        withContext(Dispatchers.IO) {
            curSocket.send(p)
        }
    }


    companion object {
        const val TAG = "UdpTransport"

        /**
         * TODO: enable editing remote address from UI
         */
        val UDP_REMOTE_ADDRESS: InetAddress = InetAddress.getByName("192.168.10.110")

        /**
         * Send packet to port 5050
         */
        const val UDP_REMOTE_PORT = 5050

        /**
         * Receive packet at port 8822
         */
        const val UDP_LOCAL_PORT = 8822
    }
}