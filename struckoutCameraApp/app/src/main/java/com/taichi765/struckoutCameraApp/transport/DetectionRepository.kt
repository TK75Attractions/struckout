package com.taichi765.struckoutCameraApp.transport

/**
 * Repository to deal with detected objects.
 */
interface DetectionRepository {
    suspend fun pushDetection(data: DetectionData)
}