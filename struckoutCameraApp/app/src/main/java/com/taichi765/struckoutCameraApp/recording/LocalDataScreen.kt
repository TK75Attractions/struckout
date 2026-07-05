package com.taichi765.struckoutCameraApp.recording

import androidx.compose.foundation.background
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import com.taichi765.struckoutCameraApp.network.LocalDetectionUploader.UploadError
import com.taichi765.struckoutCameraApp.recording.LocalDataViewModel.UploadStatus
import java.io.IOException

@Composable
fun LocalDataScreenRoute() {
    val viewModel = hiltViewModel<LocalDataViewModel>()

    val rowCount by viewModel.rowCount.collectAsState()
    val uploadStatus by viewModel.uploadStatus.collectAsState()
    val showConfirmDeleteDialog by viewModel.showConfirmDeleteDialog.collectAsState()
    val (showErrorDetail, setShowErrorDetail) = remember { mutableStateOf(false) }

    LocalDataScreen(
        rowCount = rowCount,
        uploadStatus = uploadStatus,
        showConfirmDeleteDialog = showConfirmDeleteDialog,
        showErrorDetail = showErrorDetail,
        onSyncLocalDetections = viewModel::syncLocalDetections,
        onDismissDelete = viewModel::dismissDelete,
        onConfirmDelete = viewModel::confirmDelete,
        onSetShowErrorDetail = setShowErrorDetail
    )
}

@Composable
fun LocalDataScreen(
    rowCount: Int,
    uploadStatus: UploadStatus,
    showConfirmDeleteDialog: Boolean,
    showErrorDetail: Boolean,
    onSyncLocalDetections: () -> Unit,
    onDismissDelete: () -> Unit,
    onConfirmDelete: () -> Unit,
    onSetShowErrorDetail: (Boolean) -> Unit
) {
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text("records = $rowCount")
        when (uploadStatus) {
            is UploadStatus.NotStarted -> {
                Button(onClick = {
                    onSyncLocalDetections()
                }) {
                    Text("Sync local detections")
                }
            }

            is UploadStatus.InProgress -> {
                CircularProgressIndicator()
            }

            is UploadStatus.Succeed -> {
                Text("アップロードに成功しました")
            }

            is UploadStatus.Error -> {
                Text("アップロードに失敗しました")
                TextButton(onClick = { onSetShowErrorDetail(true) }) {
                    Text("詳細")
                }

                if (showErrorDetail) {
                    ErrorDetailCard(uploadStatus.error)
                }
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
private fun ErrorDetailCard(error: UploadError) {
    val text = when (error) {
        is UploadError.NotConnected -> throw NotImplementedError("FallbackViewに行くはず")
        is UploadError.WriteFailed -> "サーバーへの書き込みに失敗しました: \n${error.cause}"
        is UploadError.ReadFailed -> "サーバーからの読み込みに失敗しました: \n${error.cause}"
        is UploadError.ProtocolError -> "プロトコル実装に不備がありました。開発者に連絡してください: \n${error.description}"
        is UploadError.ServerError -> "サーバー側でエラーがありました: \n${error.description}"
    }

    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp)
            .background(Color.Red),
        shape = RoundedCornerShape(16.dp)
    ) {
        Text(text = text)
    }
}

@Composable
private fun ConfirmDeleteDialog(onDismissDelete: () -> Unit, onConfirmDelete: () -> Unit) {
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

/**
 * [LocalDataScreen] filled with dummy parameters. This should be only used from the function marked as [Preview].
 */
@Composable
private fun DummyLocalDataScreen(
    rowCount: Int,
    uploadStatus: UploadStatus,
    showConfirmDeleteDialog: Boolean = false,
    showErrorDetail: Boolean = false
) {
    LocalDataScreen(
        rowCount = rowCount,
        uploadStatus = uploadStatus,
        showConfirmDeleteDialog = showConfirmDeleteDialog,
        showErrorDetail = showErrorDetail,
        onSetShowErrorDetail = {}, onSyncLocalDetections = {},
        onDismissDelete = {},
        onConfirmDelete = {}
    )
}

@Preview
@Composable
private fun DefaultPreview() {
    DummyLocalDataScreen(
        rowCount = 100,
        uploadStatus = UploadStatus.NotStarted,
    )
}

@Preview
@Composable
private fun InProgressPreview() {
    DummyLocalDataScreen(
        rowCount = 100,
        uploadStatus = UploadStatus.InProgress,
    )
}

@Composable
@Preview
private fun ConfirmDeletePreview() {
    DummyLocalDataScreen(
        rowCount = 100,
        uploadStatus = UploadStatus.Succeed,
        showConfirmDeleteDialog = true,
    )
}

@Preview
@Composable
private fun ErrorDetailPreview() {
    DummyLocalDataScreen(
        rowCount = 100,
        uploadStatus = UploadStatus.Error(
            UploadError.WriteFailed(
                IOException(
                    "Connection aborted"
                )
            )
        ),
        showErrorDetail = true
    )
}
