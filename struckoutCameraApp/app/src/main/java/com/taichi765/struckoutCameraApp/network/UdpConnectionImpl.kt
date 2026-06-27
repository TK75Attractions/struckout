package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.network.types.SessionState
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
class UdpConnectionImpl : UdpConnection {
    private var socket = MutableStateFlow<DatagramSocket?>(null)

    /**
     * [TcpSession]で[SessionState]をstateInするとき独自スコープを使っているので、それに合わせてこちらも
     * 同じようにする
     */
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override val isConnected = socket.map {
        it != null
    }.stateIn(
        scope = scope,
        started = SharingStarted.Eagerly,
        initialValue = false
    )

    /**
     * Creates new UDP socket and bind it to the port for receiving data from server.
     * @return
     * Returns whether connecting is succeeded or not.
     */
    override suspend fun connect(): Boolean {
        // TODO: BindErrorとConnectionError分ける
        return withContext(Dispatchers.IO) {
            try {
                Timber.tag(TAG).i("trying to bind UDP port")
                val newSocket = DatagramSocket()
                Timber.tag(TAG).i("successfully bound UDP port")

                newSocket.connect(UDP_REMOTE_ADDRESS, UDP_REMOTE_PORT)
                Timber.tag(TAG).i("successfully connected to server via UDP")

                socket.value = newSocket
                return@withContext true
            } catch (e: IOException) {
                Timber.tag(TAG).w("failed to bind port: $e")
                return@withContext false
            }
        }
    }

    override suspend fun sendPacket(packet: Struckout.UdpPacket) {
        val curSocket = socket.value
        check(curSocket != null) {
            "UDP port must be connected to port before sending packet"
        }

        val bytes = packet.toByteArray()
        val p = DatagramPacket(bytes, bytes.size)

        withContext(Dispatchers.IO) {
            curSocket.send(p)
        }
    }

    object Factory : UdpConnection.Factory {
        override fun create(): UdpConnection {
            return UdpConnectionImpl()
        }
    }

    companion object {
        const val TAG = "UdpConnectionImpl"

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