package com.taichi765.struckoutCameraApp.network.types

import com.taichi765.struckoutCameraApp.proto.CameraBallTracker

/**
 * @param timestamp フレームの撮影時刻 (UNIX epoch からの ms)。端末の時計で付けた値
 * @param sensorTimestampNanos フレームのセンサー時刻 (ns)。カメラが付けた値そのまま。分からないときは 0
 */
data class DetectionData(
    val timestamp: Long,
    val frameId: ULong,
    val detections: List<CameraBallTracker.Detection>,
    val sensorTimestampNanos: Long = 0,
)