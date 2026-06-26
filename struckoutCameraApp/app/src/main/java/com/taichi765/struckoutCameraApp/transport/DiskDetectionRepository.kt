package com.taichi765.struckoutCameraApp.transport

import javax.inject.Inject

/**
 * Saves detections to disk (in protobuf format).
 */
class DiskDetectionRepository @Inject constructor() : DetectionRepository {
    override suspend fun pushDetection(data: DetectionData) {
        TODO("Not yet implemented")
    }
}