package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.flow.StateFlow

interface UdpConnection {
    val isConnected: StateFlow<Boolean>

    suspend fun connect(): Boolean
    suspend fun sendPacket(packet: Struckout.UdpPacket)

    interface Factory {
        fun create(): UdpConnection
    }
}