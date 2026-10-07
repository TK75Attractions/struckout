package com.taichi765.struckoutCameraApp.network.types

import com.taichi765.struckoutCameraApp.proto.CameraBallTracker

data class DetectionData(
    val timestamp: Long,
    val frameId: ULong,
    val detections: List<CameraBallTracker.Detection>
)