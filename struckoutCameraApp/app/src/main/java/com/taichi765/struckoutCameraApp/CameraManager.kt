package com.taichi765.struckoutCameraApp

import android.content.Context
import android.content.Context.CAMERA_SERVICE
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraManager
import android.util.Log
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.lifecycle.awaitInstance
import androidx.compose.ui.graphics.ImageBitmap
import com.taichi765.struckoutCameraApp.MainActivity.Companion.TAG
import com.taichi765.struckoutCameraApp.ble.FrameData
import com.taichi765.struckoutCameraApp.ble.FrameID
import com.taichi765.struckoutCameraApp.imgproc.opencv.MyAnalyzer
import com.taichi765.struckoutCameraApp.imgproc.opencv.ObjectTracker
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import org.opencv.core.Core
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Rect

class CameraManager(
    context: Context,
    val onUpdateImage: (ImageBitmap) -> Unit,
    val onUpdateFrameData: (FrameData) -> Unit
) {

    private var frameId = FrameID(0u)

    val analyzer by lazy {
        MyAnalyzer(tracker) { image, rects ->
            onUpdateImage(image)
            val worldDirection = calculator.calc(rects[0])//TODO: オブジェクトが複数ある場合の処理
            val frameData =
                FrameData(frameId, worldDirection.x, worldDirection.y, worldDirection.z)
            onUpdateFrameData(frameData)
        }
    }

    private val calculator by lazy { WorldDirectionCalculator(cameraMatrix, cameraRotation) }
    val tracker = ObjectTracker(0.5, 15.0, 80.0)
    val cameraProvider by lazy {
        CoroutineScope(Dispatchers.Default).async {
            Log.i(TAG, "Initialized camera provider")
            ProcessCameraProvider.awaitInstance(context)
        }
    }

    val cameraManager by lazy {
        context.getSystemService(CAMERA_SERVICE) as CameraManager
    }

    val characteristics by lazy {
        cameraManager.cameraIdList.map { id -> cameraManager.getCameraCharacteristics(id) }
            .filter { ch -> ch.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_BACK }
    }

    val cameraMatrix: Mat by lazy {
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

    val cameraRotation by lazy {
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


    data class CameraIntrinsics(
        val fx: Float,
        val fy: Float,
        val cx: Float,
        val cy: Float,
        val s: Float
    )
}

class WorldDirectionCalculator(val cameraMatrix: Mat, val cameraRotation: Mat) {
    fun calc(rect: Rect): WorldDirection {
        val pixel = Mat(3, 1, CvType.CV_64F)

        pixel.put(
            0, 0,
            (rect.x + rect.width / 2).toDouble()
        )
        pixel.put(1, 0, (rect.y + rect.height / 2).toDouble())
        pixel.put(2, 0, 1.0)

        val normalized = Mat()
        Core.gemm(cameraMatrix.inv(), pixel, 1.0, Mat(), 0.0, normalized)

        val worldDirection = Mat()
        Core.gemm(cameraRotation, normalized, 1.0, Mat(), 0.0, worldDirection)
        print(worldDirection)
        TODO()
    }
}

data class WorldDirection(val x: Float, val y: Float, val z: Float)

