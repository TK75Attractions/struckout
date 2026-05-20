package com.taichi765.struckoutCameraApp.camera

import android.content.Context
import android.content.Context.CAMERA_SERVICE
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraManager
import android.util.Log
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.lifecycle.awaitInstance
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ViewModel
import com.taichi765.struckoutCameraApp.MainViewModel.Companion.TAG
import com.taichi765.struckoutCameraApp.ble.FrameData
import com.taichi765.struckoutCameraApp.ble.FrameID
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import org.opencv.core.Core
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Rect
import java.util.concurrent.Executors

class CameraViewModel(
    context: Context,
    val onUpdateImage: (ImageBitmap) -> Unit,
    val onUpdateFrameData: (FrameData) -> Unit
) : ViewModel() {
    var contoursImage by mutableStateOf<ImageBitmap?>(null)

    private var frameId = FrameID(0u)
    val tracker = ObjectTracker(0.5, 15.0, 80.0)


    val analyzer = MyAnalyzer(tracker) { image, rects ->
        onUpdateImage(image)
        val worldDirection = calculator.calc(rects[0])//TODO: オブジェクトが複数ある場合の処理
        val frameData =
            FrameData(frameId, worldDirection.x, worldDirection.y, worldDirection.z)
        onUpdateFrameData(frameData)
    }

    val cameraProvider = CoroutineScope(Dispatchers.Default).async {
        val ret = ProcessCameraProvider.awaitInstance(context)
        Log.i(TAG, "Initialized camera provider")
        ret
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

    suspend fun setupCamera(lifecycleOwner: LifecycleOwner) {
        val imageAnalysis = ImageAnalysis.Builder().build()
        val executor = Executors.newSingleThreadExecutor()
        imageAnalysis.setAnalyzer(executor, analyzer)

        cameraProvider.await().bindToLifecycle(
            lifecycleOwner,
            CameraSelector.DEFAULT_BACK_CAMERA,
            imageAnalysis
        )
        Log.i(TAG, "Initialized ImageAnalyzer")
    }

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

