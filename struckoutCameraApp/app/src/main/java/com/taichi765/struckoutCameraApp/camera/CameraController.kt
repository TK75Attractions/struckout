package com.taichi765.struckoutCameraApp.camera

import android.content.Context
import android.content.Context.CAMERA_SERVICE
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraManager
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Rect

class CameraController(context: Context) : CameraRepository {
    override val tracker = ObjectTracker(0.5, 15.0, 80.0)

    override fun calc(rect: Rect): WorldDirection {
        return calculator.calc(rect)
    }


    val cameraManager =
        context.getSystemService(CAMERA_SERVICE) as CameraManager


    val characteristics =
        cameraManager.cameraIdList.map { id -> cameraManager.getCameraCharacteristics(id) }
            .filter { ch -> ch.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_BACK }


    val cameraMatrix: Mat = run {
        val intrinsics =
            characteristics.mapNotNull { it.get(CameraCharacteristics.LENS_INTRINSIC_CALIBRATION) }
                .map { CameraIntrinsics(it[0], it[1], it[2], it[3], it[4]) }.toList().let {
                    assert(it.count() == 1) { "There were multiple back camera. Unable to select one." }
                    it[0]
                }

        val mtx = Mat.eye(3, 3, CvType.CV_64F)
        mtx.put(0, 0, intrinsics.fx.toDouble())
        mtx.put(1, 1, intrinsics.fy.toDouble())
        mtx.put(0, 2, intrinsics.cx.toDouble())
        mtx.put(1, 2, intrinsics.cy.toDouble())
        mtx
    }

    val cameraRotation = run {
        val rotations = characteristics
            .mapNotNull { it.get(CameraCharacteristics.LENS_POSE_ROTATION) }

        require(rotations.size == 1) {
            "There were multiple back camera. Unable to select one."
        }

        val rotation = rotations.single().map { it.toDouble() }

        Mat(4, 1, CvType.CV_64F).apply {
            put(0, 0, rotation[0])
            put(1, 0, rotation[1])
            put(2, 0, rotation[2])
            put(3, 0, rotation[3])
        }
    }

    private val calculator = WorldDirectionCalculator(cameraMatrix, cameraRotation)

    private data class CameraIntrinsics(
        val fx: Float,
        val fy: Float,
        val cx: Float,
        val cy: Float,
        val s: Float
    )


}