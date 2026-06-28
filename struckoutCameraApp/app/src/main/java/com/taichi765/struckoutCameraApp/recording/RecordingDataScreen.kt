package com.taichi765.struckoutCameraApp.recording

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.hilt.navigation.compose.hiltViewModel

@Composable
fun RecordingDataScreen() {
    val viewModel = hiltViewModel<RecordingDataViewModel>()
    val rowCount by viewModel.rowCount.collectAsState()
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text("records = $rowCount")
        Button(onClick = {
            viewModel.syncLocalDetections()
        }) {
            Text("Sync local detections")
        }
    }
}