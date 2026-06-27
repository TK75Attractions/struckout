package com.taichi765.struckoutCameraApp.recording

import com.taichi765.struckoutCameraApp.network.DetectionRepository
import com.taichi765.struckoutCameraApp.network.types.DetectionData
import javax.inject.Inject

/**
 * Saves detections to disk (in protobuf format).
 */
class DiskDetectionRepository @Inject constructor() : DetectionRepository {
    override suspend fun pushDetection(data: DetectionData) {
        TODO("Not yet implemented")
    }
}