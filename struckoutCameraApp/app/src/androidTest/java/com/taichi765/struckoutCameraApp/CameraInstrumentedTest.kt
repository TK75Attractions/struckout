package com.taichi765.struckoutCameraApp

import androidx.test.ext.junit.runners.AndroidJUnit4
import com.taichi765.struckoutCameraApp.camera.WorldDirectionCalculator
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.opencv.android.OpenCVLoader
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Rect
import kotlin.math.PI

@RunWith(AndroidJUnit4::class)
class CameraInstrumentedTest {
    @Test
    fun worldDirectionCalculator_calculatesProperly() {
        OpenCVLoader.initLocal()
        val cameraMatrix = Mat.eye(3, 3, CvType.CV_64F)
        val sensorToCameraRotation = Mat.zeros(3, 1, CvType.CV_64F).apply {
            put(2, 0, PI / 2)
        }
        val calculator = WorldDirectionCalculator(cameraMatrix, sensorToCameraRotation)

        val deviceDirection = calculator.calc(Rect(0, 0, 0, 2))

        assertEquals(1.0, deviceDirection.x, 1e-9)
        assertEquals(0.0, deviceDirection.y, 1e-9)
        assertEquals(1.0, deviceDirection.z, 1e-9)
    }
}