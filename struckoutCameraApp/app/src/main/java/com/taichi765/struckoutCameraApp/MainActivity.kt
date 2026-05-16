package com.taichi765.struckoutCameraApp

import android.Manifest
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraManager
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.lifecycle.awaitInstance
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.core.app.ActivityCompat
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.lifecycleScope
import com.taichi765.struckoutCameraApp.imgproc.opencv.MyAnalyzer
import com.taichi765.struckoutCameraApp.imgproc.opencv.ObjectTracker
import com.taichi765.struckoutCameraApp.ui.CameraPreview
import com.taichi765.struckoutCameraApp.ui.ContoursPreview
import com.taichi765.struckoutCameraApp.ui.theme.StruckoutCameraAppTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.launch
import org.opencv.android.OpenCVLoader
import org.opencv.core.Core
import org.opencv.core.CvType
import org.opencv.core.Mat
import java.util.concurrent.Executors

class MainActivity : ComponentActivity() {
    private var contoursImage by mutableStateOf<ImageBitmap?>(null)
    val analyzer by lazy {
        MyAnalyzer(tracker) { image, rects ->
            lifecycleScope.launch {
                contoursImage = image
            }
            val worldDirections = rects.map { rect ->
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
                Core.gemm(cameraRotation, normalized, 1.0, Mat(), 0.0, TODO())
                worldDirection
            }
        }
    }
    val tracker = ObjectTracker(0.5, 15.0, 80.0)
    val cameraProvider by lazy {
        CoroutineScope(Dispatchers.Default).async {
            Log.i(TAG, "Initialized camera provider")
            ProcessCameraProvider.awaitInstance(this@MainActivity)
        }
    }

    val cameraManager by lazy {
        getSystemService(CAMERA_SERVICE) as CameraManager
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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        OpenCVLoader.initLocal()
        ActivityCompat.requestPermissions(
            this,
            arrayOf(Manifest.permission.CAMERA),
            101
        )

        lifecycleScope.launch { setupCamera() }

        setContent {
            StruckoutCameraAppTheme {
                Column(
                    modifier = Modifier.fillMaxSize(),
                    verticalArrangement = Arrangement.Center
                ) {
                    Text("Camera Preview")
                    CameraPreview(
                        context = this@MainActivity,
                        cameraProvider = cameraProvider,
                        modifier = Modifier
                            .fillMaxWidth()
                            .weight(1f)
                    )

                    Text("Contours Preview")
                    ContoursPreview(
                        image = contoursImage,
                        modifier = Modifier
                            .fillMaxWidth()
                            .weight(1f)
                    )
                }
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        // TODO: close NativeAndroidEnvironment
    }

    private suspend fun setupCamera() {
        val imageAnalysis = ImageAnalysis.Builder().build()
        val executor = Executors.newSingleThreadExecutor()
        imageAnalysis.setAnalyzer(executor, analyzer)

        cameraProvider.await().bindToLifecycle(
            this as LifecycleOwner,
            CameraSelector.DEFAULT_BACK_CAMERA,
            imageAnalysis
        )
        Log.i(TAG, "Initialized ImageAnalyzer")
    }

    companion object {
        const val TAG = "MainActivity"
    }
}

data class CameraIntrinsics(
    val fx: Float,
    val fy: Float,
    val cx: Float,
    val cy: Float,
    val s: Float
)

