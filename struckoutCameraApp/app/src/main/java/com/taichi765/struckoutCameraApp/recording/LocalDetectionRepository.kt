package com.taichi765.struckoutCameraApp.recording

import com.taichi765.struckoutCameraApp.network.DetectionRepository
import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.proto.udpPacket
import javax.inject.Inject

/**
 * Saves detections to disk (in protobuf format).
 */
class LocalDetectionRepository @Inject constructor(private val frameDao: FrameDao) :
    DetectionRepository {
    override suspend fun pushDetection(data: DetectionData) {
        frameDao.insertFrame(
            FrameEntity(
                timestamp = data.timestamp,
                data = udpPacket {
                    cameraId = DUMMY_CAMERA_ID
                    timestamp = data.timestamp
                    frameId = data.frameId.toLong()
                    data.detections.forEach {
                        detectedObjects += it
                    }
                }
            )
        )
    }

    suspend fun sync() {

    }

    companion object {
        const val DUMMY_CAMERA_ID = 99
    }
}