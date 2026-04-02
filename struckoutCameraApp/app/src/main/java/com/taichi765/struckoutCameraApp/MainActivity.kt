package com.taichi765.struckoutCameraApp

import android.content.Context
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.lifecycle.awaitInstance
import androidx.camera.view.PreviewView
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.app.ActivityCompat
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.lifecycleScope
import com.taichi765.struckoutCameraApp.imgproc.opencv.MyAnalyzer
import com.taichi765.struckoutCameraApp.imgproc.opencv.ObjectTracker
import com.taichi765.struckoutCameraApp.ui.theme.StruckoutCameraAppTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.launch
import org.opencv.android.OpenCVLoader
import java.util.concurrent.Executors

class MainActivity : ComponentActivity() {
    private var contoursImage by mutableStateOf<ImageBitmap?>(null)
    val analyzer by lazy {
        MyAnalyzer(tracker) { image, rects ->
            lifecycleScope.launch {
                contoursImage = image
            }
        }
    }
    val tracker = ObjectTracker(0.5, 15.0, 80.0)
    val cameraProvider by lazy {
        CoroutineScope(Dispatchers.Default).async {
            Log.i("MainActivity", "Initialized camera provider")
            ProcessCameraProvider.awaitInstance(this@MainActivity)
        }
    }


    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        OpenCVLoader.initLocal()
        ActivityCompat.requestPermissions(
            this,
            arrayOf(android.Manifest.permission.CAMERA),
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


    private suspend fun setupCamera() {
        val imageAnalysis = ImageAnalysis.Builder().build()
        val executor = Executors.newSingleThreadExecutor()
        imageAnalysis.setAnalyzer(executor, analyzer)

        cameraProvider.await().bindToLifecycle(
            this as LifecycleOwner,
            CameraSelector.DEFAULT_BACK_CAMERA,
            imageAnalysis
        )
        Log.i("MainActivity", "Initialized ImageAnalyzer")
    }
}


@Composable
fun ContoursPreview(
    image: ImageBitmap?,
    modifier: Modifier = Modifier,
) {
    image?.let {
        Image(
            bitmap = it,
            contentDescription = null,
            modifier = modifier,
            contentScale = ContentScale.Fit,
        )
    }
}

@Composable
fun CameraPreview(
    context: Context,
    cameraProvider: Deferred<ProcessCameraProvider>,
    modifier: Modifier = Modifier,
) {
    val previewView = remember { PreviewView(context) }

    var provider: ProcessCameraProvider? by remember { mutableStateOf(null) }

    AndroidView(factory = { previewView }, modifier = modifier)

    LaunchedEffect(Unit) {
        provider = cameraProvider.await()
    }

    provider?.let {
        LaunchedEffect(Unit) {
            val preview =
                Preview.Builder().build().apply { surfaceProvider = previewView.surfaceProvider }
            it.bindToLifecycle(
                context as LifecycleOwner,
                CameraSelector.DEFAULT_BACK_CAMERA,
                preview
            )
            Log.i("MainActivity", "Initialized CameraPreview")
        }
    }
}
