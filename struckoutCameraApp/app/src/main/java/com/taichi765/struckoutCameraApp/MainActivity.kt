package com.taichi765.struckoutCameraApp

import android.Manifest
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import androidx.core.app.ActivityCompat
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.launch
import org.opencv.android.OpenCVLoader
import java.util.concurrent.Executors

class MainActivity : ComponentActivity() {
    private var contoursImage by mutableStateOf<ImageBitmap?>(null)


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
            AppTheme {

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


