package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
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
 * [NetworkManager]でインスタンスのライフサイクルを管理しているので他のところで
 * 直接使うべからず
 */
class UdpConnection {
    private var socket = MutableStateFlow<DatagramSocket?>(null)

    /**
     * [TcpSession]で[SessionState]をstateInするとき独自スコープを使っているので、それに合わせてこちらも
     * 同じようにする
     */
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    /**
     * Whether the socket is bound to a port or not.
     */
    val isBound = socket.map {
        it != null
    }.stateIn(
        scope = scope,
        started = SharingStarted.Eagerly,
        initialValue = false
    )

    /**
     * Creates new UDP socket and bind it to the port for receiving data from server.
     * @return
     * Returns whether binding is succeeded or not.
     */
    suspend fun bind(): Boolean {
        Timber.tag(TAG).i("trying to bind UDP port")
        return withContext(Dispatchers.IO) {
            try {
                val newSocket = DatagramSocket()
                socket.value = newSocket
                newSocket.connect(UDP_REMOTE_ADDRESS, UDP_REMOTE_PORT)
                Timber.tag(TAG).i("successfully bound UDP port")
                return@withContext true
            } catch (e: IOException) {
                Timber.tag(TAG).w("failed to bind port: $e")
                return@withContext false
            }
        }
    }

    suspend fun sendPacket(packet: Struckout.UdpPacket) {
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
    }
}