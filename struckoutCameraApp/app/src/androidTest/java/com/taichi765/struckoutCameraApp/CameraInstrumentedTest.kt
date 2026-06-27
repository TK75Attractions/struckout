package com.taichi765.struckoutCameraApp

import androidx.test.ext.junit.runners.AndroidJUnit4
import com.taichi765.struckoutCameraApp.camera.WorldDirectionCalculator
import org.junit.Test
import org.junit.runner.RunWith
import org.opencv.android.OpenCVLoader
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Rect

@RunWith(AndroidJUnit4::class)
class CameraInstrumentedTest {
    @Test
    fun worldDirectionCalculator_calculatesProperly() {
        OpenCVLoader.initLocal()
        val mtx = Mat.eye(3, 3, CvType.CV_64F).apply {
            put(0, 0, 0.5) // fx
            put(1, 1, 0.2) // fy
            put(0, 2, 0.01)// cx
            put(1, 2, 0.0) // cy
        }

        val rot = Mat.eye(3, 1, CvType.CV_64F).apply {
            put(0, 0, 0.0)
            put(0, 1, 0.1)
            put(0, 2, 0.05)
        }
        val calculator = WorldDirectionCalculator(mtx, rot)

        val rect = Rect(400, 400, 50, 20)
        val worldDirection = calculator.calc(rect)
        // TODO: assert here
    }
}