package com.taichi765.struckoutCameraApp.recording

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel

@Composable
fun RecordingDataScreenRoute() {
    val viewModel = hiltViewModel<RecordingDataViewModel>()

    val rowCount by viewModel.rowCount.collectAsState()
    val syncInProgress by viewModel.syncInProgress.collectAsState()
    val showConfirmDeleteDialog by viewModel.showConfirmDeleteDialog.collectAsState()

    RecordingDataScreen(
        rowCount = rowCount,
        syncInProgress = syncInProgress,
        showConfirmDeleteDialog = showConfirmDeleteDialog,
        onSyncLocalDetections = viewModel::syncLocalDetections,
        onDismissDelete = viewModel::dismissDelete,
        onConfirmDelete = viewModel::confirmDelete
    )
}

@Composable
fun RecordingDataScreen(
    rowCount: Int,
    syncInProgress: Boolean,
    showConfirmDeleteDialog: Boolean,
    onSyncLocalDetections: () -> Unit,
    onDismissDelete: () -> Unit,
    onConfirmDelete: () -> Unit
) {
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text("records = $rowCount")
        if (syncInProgress) {
            CircularProgressIndicator()
        } else {
            Button(onClick = {
                onSyncLocalDetections()
            }) {
                Text("Sync local detections")
            }
        }
    }

    if (showConfirmDeleteDialog) {
        ConfirmDeleteDialog(
            onDismissDelete = onDismissDelete,
            onConfirmDelete = onConfirmDelete
        )
    }
}

@Composable
fun ConfirmDeleteDialog(onDismissDelete: () -> Unit, onConfirmDelete: () -> Unit) {
    Dialog(onDismissRequest = onDismissDelete) {
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .height(375.dp)
                .padding(16.dp),
            shape = RoundedCornerShape(16.dp)
        ) {
            Column(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Text("Succeeded to upload local detection data")
                Text("アップロード済みのデータをローカルから消去しますか？")
                Row(modifier = Modifier.fillMaxWidth()) {
                    TextButton(onClick = onDismissDelete) {
                        Text("いいえ")
                    }
                    TextButton(onClick = onConfirmDelete) {
                        Text("はい")
                    }
                }
            }
        }
    }
}

@Preview
@Composable
fun DefaultPreview() {
    RecordingDataScreen(
        rowCount = 100,
        syncInProgress = false,
        showConfirmDeleteDialog = false,
        onSyncLocalDetections = {},
        onDismissDelete = {},
        onConfirmDelete = {})
}

@Preview
@Composable
fun SyncInProgressPreview() {
    RecordingDataScreen(
        rowCount = 100,
        syncInProgress = true,
        showConfirmDeleteDialog = false,
        onSyncLocalDetections = {},
        onConfirmDelete = {},
        onDismissDelete = {})
}

@Composable
@Preview
fun ConfirmDeletePreview() {
    RecordingDataScreen(
        rowCount = 100,
        syncInProgress = false,
        showConfirmDeleteDialog = true,
        onSyncLocalDetections = {},
        onConfirmDelete = {},
        onDismissDelete = {})
}
