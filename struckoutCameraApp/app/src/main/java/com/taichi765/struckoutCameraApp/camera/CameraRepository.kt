package com.taichi765.struckoutCameraApp.camera

import android.content.Context
import android.content.Context.CAMERA_SERVICE
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraManager
import com.taichi765.struckoutCameraApp.camera.types.WorldDirection
import dagger.hilt.android.qualifiers.ApplicationContext
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Rect
import timber.log.Timber
import javax.inject.Inject
import kotlin.math.acos
import kotlin.math.sin

class CameraRepository @Inject constructor(@ApplicationContext context: Context) {
    val tracker = ObjectTracker(0.5, 15.0, 80.0)

    fun calc(rect: Rect): WorldDirection {
        return calculator.calc(rect)
    }

    private val cameraManager =
        context.getSystemService(CAMERA_SERVICE) as CameraManager


    private val characteristics = run {
        cameraManager.cameraIdList.map { id -> cameraManager.getCameraCharacteristics(id) }
            .filter { ch -> ch.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_BACK }
    }


    private val cameraMatrix: Mat = run {
        val intrinsic =
            characteristics.mapNotNull { it.get(CameraCharacteristics.LENS_INTRINSIC_CALIBRATION) }
                .map { CameraIntrinsics(it[0], it[1], it[2], it[3], it[4]) }.let {
                    if (it.count() > 1) {
                        Timber.tag(TAG).i("There were multiple back camera. selecting first one.")
                    }
                    it[0]
                }

        Mat.eye(3, 3, CvType.CV_64F).apply {
            put(0, 0, intrinsic.fx.toDouble())
            put(1, 1, intrinsic.fy.toDouble())
            put(0, 2, intrinsic.cx.toDouble())
            put(1, 2, intrinsic.cy.toDouble())
        }
    }

    private val cameraRotation = run {
        val rotations = characteristics
            .mapNotNull { it.get(CameraCharacteristics.LENS_POSE_ROTATION) }

        if (rotations.count() > 1) {
            Timber.tag(TAG).i("There were multiple back camera. selecting first one.")
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

    companion object {
        const val TAG = "CameraController"
    }
}