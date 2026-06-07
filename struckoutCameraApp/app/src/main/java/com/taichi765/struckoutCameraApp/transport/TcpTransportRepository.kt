package com.taichi765.struckoutCameraApp.transport

import kotlinx.coroutines.flow.StateFlow
import struckout.v1.Struckout

interface TcpTransportRepository {
    val state: StateFlow<ConnectionState>
    suspend fun connect(): Boolean

    suspend fun close()
    suspend fun sendPacket(packet: Struckout.TcpClientPacket)
}

sealed class ConnectionState {
    data class Connected(val cameraID: UInt) : ConnectionState()

    object Disconnected : ConnectionState()
}