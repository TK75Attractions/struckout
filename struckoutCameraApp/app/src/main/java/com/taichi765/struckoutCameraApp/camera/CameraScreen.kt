package com.taichi765.struckoutCameraApp.camera

import android.Manifest
import android.util.Log
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.viewmodel.compose.viewModel
import com.taichi765.struckoutCameraApp.camera.CameraViewModel.Companion.TAG
import java.util.concurrent.Executors

@Composable
fun CameraScreen(
    bleRepository: BleRepository
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current

    val viewModel = run {
        val cameraController = CameraController(context)
        val factory = CameraViewModel.Factory(bleRepository, cameraController)
        viewModel<CameraViewModel>(factory = factory)
    }

    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        println(it)
    }
    val image by viewModel.contoursImage.collectAsState()

    val cameraProvider by remember { mutableStateOf<ProcessCameraProvider?>(null) }

    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center
    ) {
        Text("Camera Preview")
        CameraPreview(
            cameraProvider = cameraProvider,
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
        )

        Text("Contours Preview")
        ContoursPreview(
            image = image,
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
        )
    }

    LaunchedEffect(Unit) {
        launcher.launch(Manifest.permission.CAMERA)

        val imageAnalysis =
            ImageAnalysis.Builder().build().apply {
                val executor = Executors.newSingleThreadExecutor()
                setAnalyzer(executor, viewModel.analyzer)
            }

        val cameraProvider = ProcessCameraProvider.awaitInstance(context)

        cameraProvider.bindToLifecycle(
            lifecycleOwner,
            CameraSelector.DEFAULT_BACK_CAMERA,
            imageAnalysis
        )
        Log.i(TAG, "Initialized ImageAnalyzer")
    }

    DisposableEffect(lifecycleOwner) {
        onDispose {
            cameraProvider?.unbindAll()
            TODO("clean up camera")
        }
    }
}

@Composable
private fun ContoursPreview(
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
private fun CameraPreview(
    cameraProvider: ProcessCameraProvider?,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val previewView = remember { PreviewView(context) }


    AndroidView(factory = { previewView }, modifier = modifier)



    cameraProvider?.let {
        LaunchedEffect(Unit) {
            val preview =
                Preview.Builder().build().apply { surfaceProvider = previewView.surfaceProvider }
            it.bindToLifecycle(
                context as LifecycleOwner,
                CameraSelector.DEFAULT_BACK_CAMERA,
                preview
            )
            Log.i("CameraPreview", "Initialized CameraPreview")
        }
    }
}