package com.taichi765.struckoutCameraApp.camera

import android.content.Context
import android.content.Context.CAMERA_SERVICE
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraManager
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Rect
import kotlin.math.acos
import kotlin.math.sin

class CameraController(context: Context) : CameraRepository {
    override val tracker = ObjectTracker(0.5, 15.0, 80.0)

    override fun calc(rect: Rect): WorldDirection {
        return calculator.calc(rect)
    }

    private val cameraManager =
        context.getSystemService(CAMERA_SERVICE) as CameraManager


    private val characteristics =
        cameraManager.cameraIdList.map { id -> cameraManager.getCameraCharacteristics(id) }
            .filter { ch -> ch.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_BACK }


    private val cameraMatrix: Mat = run {
        val intrinsics =
            characteristics.mapNotNull { it.get(CameraCharacteristics.LENS_INTRINSIC_CALIBRATION) }
                .map { CameraIntrinsics(it[0], it[1], it[2], it[3], it[4]) }.toList().let {
                    assert(it.count() == 1) { "There were multiple back camera. Unable to select one." }
                    it[0]
                }

        Mat.eye(3, 3, CvType.CV_64F).apply {
            put(0, 0, intrinsics.fx.toDouble())
            put(1, 1, intrinsics.fy.toDouble())
            put(0, 2, intrinsics.cx.toDouble())
            put(1, 2, intrinsics.cy.toDouble())
        }
    }

    private val cameraRotation = run {
        val rotations = characteristics
            .mapNotNull { it.get(CameraCharacteristics.LENS_POSE_ROTATION) }

        require(rotations.size == 1) {
            "There were multiple back camera. Unable to select one."
        }

        val rotation = rotations.single().map { it.toDouble() }

        val x = rotation[0]
        val y = rotation[1]
        val z = rotation[2]
        val w = rotation[3]

        // Convert the quaternion to rotation vector. See https://developer.android.com/reference/android/hardware/camera2/CameraCharacteristics#LENS_POSE_ROTATION
        val theta = 2 * acos(w)
        val ax = x / sin(theta / 2)
        val ay = y / sin(theta / 2)
        val az = z / sin(theta / 2)

        Mat(3, 1, CvType.CV_64F).apply {
            put(0, 0, theta * ax)
            put(1, 0, theta * ay)
            put(2, 0, theta * az)
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