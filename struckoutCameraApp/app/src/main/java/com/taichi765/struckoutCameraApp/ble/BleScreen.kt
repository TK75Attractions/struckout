package com.taichi765.struckoutCameraApp.ble

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.lifecycle.viewmodel.compose.viewModel

@Composable
fun BleScreen(viewModel: BleViewModel = viewModel()) {
    val cameraLocation by viewModel.cameraLocation.collectAsState()
    BleInfoView(
        cameraLocation = cameraLocation
    ) { viewModel.updateCameraLocation(it) }
}