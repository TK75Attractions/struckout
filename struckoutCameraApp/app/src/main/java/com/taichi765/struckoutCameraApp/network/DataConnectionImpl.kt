package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.withContext
import timber.log.Timber
import java.io.IOException
import java.io.OutputStream
import java.net.InetAddress
import java.net.Socket


/**
 * Sends detections to server via UDP.
 * [NetworkManager]でインスタンスのライフサイクルを管理しているので他のところで
 * 直接使うべからず
 */
class DataConnectionImpl : DataConnection {
    private var _connState = MutableStateFlow<ConnectionState>(ConnectionState.DisConnected)


    /**
     * [TcpSession]で[SessionState]をstateInするとき独自スコープを使っているので、それに合わせてこちらも
     * 同じようにする
     */
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override val isConnected = _connState.map {
        it is ConnectionState.Connected
    }.stateIn(
        scope = scope,
        started = SharingStarted.Eagerly,
        initialValue = false
    )

    /**
     * Creates new TCP socket and bind it to the port for sending camera data.
     * @return `null` if connection succeeds.
     */
    override suspend fun connect(): DataConnectionError? {
        return withContext(Dispatchers.IO) {
            Timber.tag(TAG).i("connecting to ball_watcher")
            try {
                val socket = Socket(REMOTE_ADDRESS, REMOTE_PORT)
                Timber.tag(TAG).i("TCP connection has been established successfully")
                _connState.value =
                    ConnectionState.Connected(socket = socket, output = socket.getOutputStream())
                return@withContext null
            } catch (e: IOException) {
                Timber.tag(TAG).w("failed to connect to TCP server: $e")
                return@withContext DataConnectionError.TcpConnectionFailed(e)
            }
        }
    }

    override suspend fun sendPacket(packet: Struckout.DetectionsPacket) {
        val curState = _connState.value
        check(curState is ConnectionState.Connected) {
            "TCP must be connected before sending packet"
        }

        withContext(Dispatchers.IO) {
            writePacket(curState.output, packet)
        }
    }

    object Factory : DataConnection.Factory {
        override fun create(): DataConnection {
            return DataConnectionImpl()
        }
    }

    private sealed interface ConnectionState {
        data class Connected(val socket: Socket, val output: OutputStream) : ConnectionState
        data object DisConnected : ConnectionState
    }

    companion object {
        const val TAG = "DataConnectionImpl"

        /**
         * TODO: enable editing remote address from UI
         */
        val REMOTE_ADDRESS: InetAddress = InetAddress.getByName("127.0.0.1")

        /**
         * Send packet to port 5050
         */
        const val REMOTE_PORT = 5050
    }
}