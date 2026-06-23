package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.proto.Struckout

/**
 * Repository to deal with detected objects.
 */
interface DetectionRepository {
    suspend fun pushDetection(packet: Struckout.UdpPacket)
}