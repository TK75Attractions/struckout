package com.taichi765.struckoutCameraApp.camera

import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.lifecycle.awaitInstance
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Button
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
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.taichi765.struckoutCameraApp.camera.CameraViewModel.Companion.TAG
import timber.log.Timber
import java.util.concurrent.Executors

@Composable
fun CameraScreenRoute(
) {
    val viewModel = hiltViewModel<CameraViewModel>()
    val image by viewModel.contoursImage.collectAsState()

    CameraScreen(
        image,
        viewModel.analyzer,
        viewModel::flashVideo
    )
}

@Composable
private fun CameraScreen(
    image: ImageBitmap?,
    analyzer: MyAnalyzer,
    onFlashVideo: () -> Unit
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current

    val cameraProvider by remember { mutableStateOf<ProcessCameraProvider?>(null) }

    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center
    ) {
        Text("Camera Preview")

        ContoursPreview(
            image = image,
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
        )

        Button(onClick = onFlashVideo) {
            Text("Stop recording")
        }
    }

    LaunchedEffect(Unit) {
        val imageAnalysis =
            ImageAnalysis.Builder().build().apply {
                val executor = Executors.newSingleThreadExecutor()
                setAnalyzer(executor, analyzer)
            }

        val cameraProvider = ProcessCameraProvider.awaitInstance(context)

        cameraProvider.bindToLifecycle(
            lifecycleOwner,
            CameraSelector.DEFAULT_BACK_CAMERA,
            imageAnalysis
        )
        Timber.tag(TAG).i("Initialized ImageAnalyzer")
    }

    DisposableEffect(lifecycleOwner) {
        onDispose {
            cameraProvider?.unbindAll()
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
