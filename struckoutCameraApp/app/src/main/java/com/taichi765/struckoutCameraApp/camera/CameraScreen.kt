package com.taichi765.struckoutCameraApp.camera

import android.hardware.camera2.CameraCaptureSession
import androidx.annotation.OptIn
import androidx.camera.camera2.interop.Camera2Interop
import androidx.camera.camera2.interop.ExperimentalCamera2Interop
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
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.taichi765.struckoutCameraApp.camera.CameraViewModel.Companion.TAG
import timber.log.Timber
import java.util.concurrent.Executors

@Composable
fun CameraScreenRoute(
) {
    val viewModel = hiltViewModel<CameraViewModel>()
    val image by viewModel.contoursImage.collectAsState()
    val savedFrame by viewModel.savedFrame.collectAsState()

    CameraScreen(
        image,
        savedFrame,
        viewModel.analyzer,
        viewModel.captureCallback,
        viewModel::requestSaveFrame,
        viewModel::flashVideo
    )
}

@OptIn(ExperimentalCamera2Interop::class)
@Composable
private fun CameraScreen(
    image: ImageBitmap?,
    savedFrame: String?,
    analyzer: MyAnalyzer,
    captureCallback: CameraCaptureSession.CaptureCallback,
    onSaveFrame: () -> Unit,
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

        // カメラの位置と向きを PC で求めるための画像を保存する。
        Button(onClick = onSaveFrame) {
            Text("Save frame")
        }
        savedFrame?.let { Text(it) }

        Button(onClick = onFlashVideo) {
            Text("Stop recording")
        }
    }

    LaunchedEffect(Unit) {
        val imageAnalysis =
            ImageAnalysis.Builder().also {
                Camera2Interop.Extender(it).setSessionCaptureCallback(captureCallback)
            }.build().apply {
                val executor = Executors.newSingleThreadExecutor()
                setAnalyzer(executor, analyzer)
            }

        val cameraProvider = ProcessCameraProvider.awaitInstance(context)

        // TODO: taichi765のスマホは60をサポートしていないので誰かに借りる
        //val sessionConfig =
        //    SessionConfig.Builder(imageAnalysis).setFrameRateRange(Range(45, 60)).build()
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
