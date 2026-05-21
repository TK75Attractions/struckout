package com.taichi765.struckoutCameraApp

import com.taichi765.struckoutCameraApp.camera.WorldDirectionCalculator
import org.junit.Test
import org.opencv.android.OpenCVLoader
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Rect

class CameraViewModelUnitTest {

    @Test
    fun worldDirectionCalculator_calculatesProperly() {
        OpenCVLoader.initLocal()
        val mtx = Mat.eye(3, 3, CvType.CV_64F).apply {
            put(0, 0, 0.5)
            put(1, 1, 0.2)
            put(0, 2, 0.01)
            put(1, 2, 0.0)
        }

        val rot = Mat.eye(1, 3, CvType.CV_64F).apply {

            put(0, 0, 0.0)
            put(0, 1, 0.1)
            put(0, 2, 0.05)
        }
        val calculator = WorldDirectionCalculator(mtx, rot)

        val rect = Rect(400, 400, 50, 20)
        val worldDirection = calculator.calc(rect)
        assert(worldDirection.x == 0f)
    }
}