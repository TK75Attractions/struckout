package com.taichi765.struckoutCameraApp.transport

import kotlinx.coroutines.flow.StateFlow
import struckout.v1.Struckout

interface TcpTransportRepository {
    val isConnected: StateFlow<Boolean>
    suspend fun connect(): Boolean

    suspend fun close()
    suspend fun sendPacket(packet: Struckout.TcpClientPacket)
}