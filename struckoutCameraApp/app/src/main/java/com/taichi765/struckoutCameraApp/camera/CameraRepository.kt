package com.taichi765.struckoutCameraApp.camera

import com.taichi765.struckoutCameraApp.camera.types.WorldDirection
import org.opencv.core.Rect

interface CameraRepository {
    val tracker: ObjectTracker
    fun calc(rect: Rect): WorldDirection
}