package com.taichi765.struckoutCameraApp.camera

import android.content.Context
import android.util.Log
import androidx.camera.core.CameraSelector
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
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
import androidx.lifecycle.LifecycleOwner
import kotlinx.coroutines.Deferred

@Composable
fun CameraView(
    context: Context,
    cameraProvider: Deferred<ProcessCameraProvider>,
    contoursImage: ImageBitmap
) {
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center
    ) {
        Text("Camera Preview")
        CameraPreview(
            context = context,
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
            Log.i("CameraPreview", "Initialized CameraPreview")
        }
    }
}