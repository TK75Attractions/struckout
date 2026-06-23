package com.taichi765.struckoutCameraApp.camera

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.lifecycle.awaitInstance
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavController
import com.taichi765.struckoutCameraApp.R
import com.taichi765.struckoutCameraApp.camera.CameraViewModel.Companion.TAG
import com.taichi765.struckoutCameraApp.transport.TcpTransportRepository
import com.taichi765.struckoutCameraApp.transport.UdpTransportRepository
import timber.log.Timber
import java.util.concurrent.Executors

@Composable
fun CameraScreen(
    udpRepository: UdpTransportRepository,
    tcpRepository: TcpTransportRepository,
    navController: NavController
) {
    val context = LocalContext.current
    var permissionGranted by remember { mutableStateOf(context.checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }

    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        permissionGranted = true
    }

    if (permissionGranted) {
        CameraScreenContent(udpRepository, tcpRepository) {
            navController.navigate("config")
        }
    } else {
        Text("Permission is required")
        LaunchedEffect(Unit) {
            launcher.launch(Manifest.permission.CAMERA)
        }
    }
}

@Composable
private fun CameraScreenContent(
    udpRepository: UdpTransportRepository,
    tcpRepository: TcpTransportRepository,
    onNavigateToSettings: () -> Unit
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current

    val viewModel = run {
        val cameraController = CameraController(context)
        val factory = CameraViewModel.Factory(udpRepository, tcpRepository, cameraController)
        viewModel<CameraViewModel>(factory = factory)
    }
    val image by viewModel.contoursImage.collectAsState()
    val udpIsBound by viewModel.udpIsBound.collectAsState()
    val cameraProvider by remember { mutableStateOf<ProcessCameraProvider?>(null) }

    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center
    ) {
        Row {
            Text("Camera Preview")

            IconButton(onClick = {
                onNavigateToSettings()
            }) {
                Icon(
                    painter = painterResource(R.drawable.settings_24px),
                    contentDescription = "settings"
                )
            }
        }

        ContoursPreview(
            image = image,
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
        )
    }

    if (!udpIsBound) {
        LaunchedEffect(Unit) {
            viewModel.bindUdpSocket()
        }
    }

    LaunchedEffect(Unit) {
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
