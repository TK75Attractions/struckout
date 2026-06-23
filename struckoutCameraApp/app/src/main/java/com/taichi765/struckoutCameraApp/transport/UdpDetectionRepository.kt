package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.withContext
import kotlinx.io.IOException
import timber.log.Timber
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress


/**
 * Sends detections to server via UDP.
 */
class UdpDetectionRepository(scope: CoroutineScope) : DetectionRepository {
    private var socket = MutableStateFlow<DatagramSocket?>(null)

    /**
     * Whether the socket is bound to a port or not.
     */
    val isBound = socket.map {
        it != null
    }.stateIn(
        scope = scope,
        started = SharingStarted.WhileSubscribed(5000),
        initialValue = false
    )// FIXME: ここでstateInするのは問題ない？

    /**
     * Creates new UDP socket and bind it to the port for receiving data from server.
     * @return
     * Returns whether binding is succeeded or not.
     */
    suspend fun bind(): Boolean {
        return withContext(Dispatchers.IO) {
            try {
                val newSocket = DatagramSocket(UDP_LOCAL_PORT)
                socket.value = newSocket
                newSocket.connect(UDP_REMOTE_ADDRESS, UDP_REMOTE_PORT)
                return@withContext true
            } catch (e: IOException) {
                Timber.tag(TAG).w("failed to bind port $UDP_LOCAL_PORT: $e")
                return@withContext false
            }
        }
    }

    override suspend fun pushDetection(packet: Struckout.UdpPacket) {
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