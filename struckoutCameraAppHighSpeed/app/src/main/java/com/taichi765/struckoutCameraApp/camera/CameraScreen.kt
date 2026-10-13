package com.taichi765.struckoutCameraApp.camera

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import com.taichi765.struckoutCameraApp.camera.highspeed.HighSpeedCamera

@Composable
fun CameraScreenRoute(
) {
    val viewModel = hiltViewModel<CameraViewModel>()
    val image by viewModel.contoursImage.collectAsState()
    val cameraState by viewModel.cameraState.collectAsState()
    val cameraStats by viewModel.cameraStats.collectAsState()
    val savedFrame by viewModel.savedFrame.collectAsState()

    CameraScreen(
        image,
        savedFrame,
        cameraState,
        cameraStats,
        viewModel::requestSaveFrame,
        viewModel::flashVideo
    )

    // 画面を離れたりアプリが裏に回ったりしたらカメラを手放す。
    LifecycleResumeEffect(Unit) {
        viewModel.startCamera()
        onPauseOrDispose {
            viewModel.stopCamera()
        }
    }
}

@Composable
private fun CameraScreen(
    image: ImageBitmap?,
    savedFrame: String?,
    cameraState: HighSpeedCamera.State,
    cameraStats: HighSpeedCamera.Stats?,
    onSaveFrame: () -> Unit,
    onFlashVideo: () -> Unit
) {
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center
    ) {
        Text("Camera Preview (high speed)")
        Text(cameraStateText(cameraState))
        cameraStats?.let { Text(cameraStatsText(it)) }

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
}

private fun cameraStateText(state: HighSpeedCamera.State): String = when (state) {
    HighSpeedCamera.State.Stopped -> "停止中"
    HighSpeedCamera.State.Starting -> "起動中"
    is HighSpeedCamera.State.Running ->
        "${state.fps}fps  撮影 ${state.streamSize}  解析 ${state.analysisSize}"

    is HighSpeedCamera.State.Failed -> "高速撮影を開始できません: ${state.message}"
}

private fun cameraStatsText(stats: HighSpeedCamera.Stats): String =
    "カメラ %.1ffps  解析 %.1ffps  取りこぼし %d\n読み出し %.1fms  解析 %.1fms".format(
        stats.cameraFps,
        stats.analyzedFps,
        stats.droppedFrames,
        stats.readbackMillis,
        stats.analysisMillis
    )

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
