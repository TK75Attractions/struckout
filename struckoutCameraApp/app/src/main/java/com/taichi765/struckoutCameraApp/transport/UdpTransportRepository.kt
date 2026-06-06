package com.taichi765.struckoutCameraApp.transport

import kotlinx.coroutines.flow.StateFlow
import struckout.v1.Struckout

interface UdpTransportRepository {
    val isBound: StateFlow<Boolean>

    /**
     * Creates new UDP socket and bind it to the port for receiving data from server.
     * @return
     * Returns whether binding is succeeded or not.
     */
    suspend fun bind(): Boolean

    suspend fun sendPacket(packet: Struckout.UdpPacket)
}