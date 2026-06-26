package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.proto.Struckout

data class DetectionData(
    val timestamp: Long,
    val frameId: ULong,
    val detections: List<Struckout.DetectedObject>
)