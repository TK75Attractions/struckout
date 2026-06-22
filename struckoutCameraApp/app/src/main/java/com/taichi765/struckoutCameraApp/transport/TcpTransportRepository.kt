package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.flow.Flow

interface TcpTransportRepository {
    val state: Flow<ConnectionState>
    suspend fun connect(): Boolean

    suspend fun close()
    suspend fun sendPacket(packet: Struckout.TcpClientPacket)
}

sealed interface ConnectionState {
    data class Connected(val cameraID: UInt) : ConnectionState

    object Disconnected : ConnectionState
}