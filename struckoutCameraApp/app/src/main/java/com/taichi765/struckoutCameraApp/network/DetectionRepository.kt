package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.network.types.DetectionData

/**
 * Repository to deal with detected objects.
 */
interface DetectionRepository {
    suspend fun pushDetection(data: DetectionData)
}