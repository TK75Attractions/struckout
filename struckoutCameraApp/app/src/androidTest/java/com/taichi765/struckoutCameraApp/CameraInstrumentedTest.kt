package com.taichi765.struckoutCameraApp

import androidx.test.ext.junit.runners.AndroidJUnit4
import com.taichi765.struckoutCameraApp.camera.WorldDirectionCalculator
import com.taichi765.struckoutCameraApp.camera.defaultBackCameraRotation
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.opencv.android.OpenCVLoader
import org.opencv.calib3d.Calib3d
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

    @Test
    fun worldDirectionCalculator_usesDefaultBackCameraPose() {
        OpenCVLoader.initLocal()
        val cameraMatrix = Mat.eye(3, 3, CvType.CV_64F)
        val rotationMatrix = Mat(3, 3, CvType.CV_64F).apply {
            put(0, 0, *defaultBackCameraRotation(90))
        }
        val sensorToCameraRotation = Mat().also { Calib3d.Rodrigues(rotationMatrix, it) }
        val calculator = WorldDirectionCalculator(cameraMatrix, sensorToCameraRotation)

        // 画像の中心は端末の真後ろ
        val center = calculator.calc(Rect(0, 0, 0, 0))
        assertEquals(0.0, center.x, 1e-9)
        assertEquals(0.0, center.y, 1e-9)
        assertEquals(-1.0, center.z, 1e-9)

        // 画像の右は端末の下
        val right = calculator.calc(Rect(1, 0, 0, 0))
        assertEquals(0.0, right.x, 1e-9)
        assertEquals(-1.0, right.y, 1e-9)
        assertEquals(-1.0, right.z, 1e-9)

        // 画像の下は端末の左
        val below = calculator.calc(Rect(0, 1, 0, 0))
        assertEquals(-1.0, below.x, 1e-9)
        assertEquals(0.0, below.y, 1e-9)
        assertEquals(-1.0, below.z, 1e-9)
    }
}