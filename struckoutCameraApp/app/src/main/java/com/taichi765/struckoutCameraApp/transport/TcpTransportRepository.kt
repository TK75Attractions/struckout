package com.taichi765.struckoutCameraApp.transport

import struckout.Struckout

interface TcpTransportRepository {
    suspend fun sendPacket(packet: Struckout.TcpClientPacket)
}