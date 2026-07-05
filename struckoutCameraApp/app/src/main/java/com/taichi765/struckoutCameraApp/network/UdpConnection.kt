package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.flow.StateFlow
import java.net.SocketException

interface UdpConnection {
    val isConnected: StateFlow<Boolean>

    /**
     * @return `null` if connection succeeds.
     */
    suspend fun connect(): UdpConnectionError?
    suspend fun sendPacket(packet: Struckout.DetectionsPacket)

    interface Factory {
        fun create(): UdpConnection
    }
}

/**
 * Error returned from [UdpConnection.connect].
 *
 * 今は一種類しか無いが後で拡張するかもしれないのでsealed interfaceにしておく.
 */
sealed interface UdpConnectionError {
    data class UdpSocketError(val cause: SocketException) : UdpConnectionError
}