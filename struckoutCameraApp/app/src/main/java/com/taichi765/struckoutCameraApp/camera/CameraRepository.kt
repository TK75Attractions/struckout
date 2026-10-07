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

    /**
     * @param imageWidth,imageHeight [rect] を検出した解析画像 (回転前) の大きさ
     */
    fun calc(rect: Rect, imageWidth: Int, imageHeight: Int): WorldDirection {
        return calculatorFor(imageWidth, imageHeight).calc(rect)
    }

    private val cameraManager =
        context.getSystemService(CAMERA_SERVICE) as CameraManager

    // CameraScreen は DEFAULT_BACK_CAMERA で撮るので、同じく最初の背面カメラを見る。
    private val characteristics = run {
        val backCameras =
            cameraManager.cameraIdList.map { id -> cameraManager.getCameraCharacteristics(id) }
                .filter { ch -> ch.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_BACK }
        if (backCameras.count() > 1) {
            Timber.tag(TAG).d("There were multiple back camera. selecting first one.")
        }
        backCameras.firstOrNull()
            ?: throw IllegalStateException("This device does not have a back camera.")
    }

    // LENS_INTRINSIC_CALIBRATION は pre-correction active array の座標系で表される。
    private val activeArraySize = run {
        characteristics.get(CameraCharacteristics.SENSOR_INFO_PRE_CORRECTION_ACTIVE_ARRAY_SIZE)
            ?: characteristics.get(CameraCharacteristics.SENSOR_INFO_ACTIVE_ARRAY_SIZE)
            ?: throw IllegalStateException("This device does not report the active array size.")
    }

    /**
     * Intrinsics in the coordinate system of [activeArraySize].
     */
    private val sensorIntrinsics: CameraIntrinsics = run {
        val calibration = characteristics.get(CameraCharacteristics.LENS_INTRINSIC_CALIBRATION)
            ?: throw IllegalStateException(
                "This device does not support LENS_INTRINSIC_CALIBRATION." +
                        "You may need to manually measure intrinsics."
            )
        CameraIntrinsics(
            fx = calibration[0].toDouble(),
            fy = calibration[1].toDouble(),
            cx = calibration[2].toDouble(),
            cy = calibration[3].toDouble()
        )
    }

    private val cameraRotation: Mat = run {
        val rotation = characteristics.get(CameraCharacteristics.LENS_POSE_ROTATION)
            ?.map { it.toDouble() }
            ?: throw IllegalStateException("This device does not support LENS_POSE_ROTATION.")

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

    private var calculator: SizedCalculator? = null

    /**
     * 検出座標は解析画像のピクセルなので、カメラ行列もその解像度に合わせて作る。
     * 解像度は端末が決めるため、最初のフレームが来るまで分からない。
     */
    @Synchronized
    private fun calculatorFor(imageWidth: Int, imageHeight: Int): WorldDirectionCalculator {
        calculator?.let {
            if (it.imageWidth == imageWidth && it.imageHeight == imageHeight) return it.calculator
        }

        val intrinsics = sensorIntrinsics.scaledToImage(
            activeArrayWidth = activeArraySize.width(),
            activeArrayHeight = activeArraySize.height(),
            imageWidth = imageWidth,
            imageHeight = imageHeight
        )
        Timber.tag(TAG).i("camera intrinsics for ${imageWidth}x$imageHeight: $intrinsics")
        val cameraMatrix = Mat.eye(3, 3, CvType.CV_64F).apply {
            put(0, 0, intrinsics.fx)
            put(1, 1, intrinsics.fy)
            put(0, 2, intrinsics.cx)
            put(1, 2, intrinsics.cy)
        }
        return WorldDirectionCalculator(cameraMatrix, cameraRotation).also {
            calculator = SizedCalculator(imageWidth, imageHeight, it)
        }
    }

    private data class SizedCalculator(
        val imageWidth: Int,
        val imageHeight: Int,
        val calculator: WorldDirectionCalculator
    )

    companion object {
        const val TAG = "CameraController"
    }
}
