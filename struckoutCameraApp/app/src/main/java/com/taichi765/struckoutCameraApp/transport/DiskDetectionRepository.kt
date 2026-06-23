package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.proto.Struckout

/**
 * Saves detections to disk (in protobuf format).
 */
class DiskDetectionRepository : DetectionRepository {
    override suspend fun pushDetection(packet: Struckout.UdpPacket) {
        TODO("Not yet implemented")
    }
}