package com.taichi765.struckoutCameraApp.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.network.LocalDetectionUploader
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class LocalDataViewModel @Inject constructor(
    private val localDetectionRepository: LocalDetectionRepository,
    private val localDetectionUploader: LocalDetectionUploader
) : ViewModel() {
    val rowCount = localDetectionRepository.rowCount.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(5000),
        initialValue = 0
    )

    private val _connectionStatus = MutableStateFlow<ConnectionStatus>(ConnectionStatus.NoAttempts)
    val connectionStatus = _connectionStatus.asStateFlow()

    private val _uploadStatus = MutableStateFlow<UploadStatus>(UploadStatus.NotStarted)
    val uploadStatus = _uploadStatus.asStateFlow()

    private val _showConfirmDeleteDialog = MutableStateFlow(false)
    val showConfirmDeleteDialog = _showConfirmDeleteDialog.asStateFlow()


    init {
        connect()
    }

    fun uploadLocalDetections() {
        viewModelScope.launch {
            _uploadStatus.value = UploadStatus.InProgress
            val frames = localDetectionRepository.loadAll()
            val error = localDetectionUploader.upload(frames)
            if (error == null) {
                _uploadStatus.value = UploadStatus.Succeed
            } else {
                // TODO: エラーの種類に応じてログとかやる
                _uploadStatus.value = UploadStatus.Error(error)
            }
        }

        _showConfirmDeleteDialog.value = true
    }

    fun dismissDelete() {
        _showConfirmDeleteDialog.value = false
    }

    fun confirmDelete() {
        _showConfirmDeleteDialog.value = false
        viewModelScope.launch {
            try {
                localDetectionRepository.deleteAll()
            } catch (e: Exception) {
                TODO()
            }
        }
    }

    /**
     * Resets [UploadStatus] before leaving screen.
     */
    fun leaveScreen() {
        _uploadStatus.value = UploadStatus.NotStarted
    }

    fun connect() {
        viewModelScope.launch {
            val error = localDetectionUploader.connect()
            if (error != null) {
                _connectionStatus.value = ConnectionStatus.Error(error)
            } else {
                _connectionStatus.value = ConnectionStatus.Connected
            }
        }
    }

    sealed interface UploadStatus {
        data object NotStarted : UploadStatus

        // TODO: ここで何分の何進んだか管理してもいいかも
        data object InProgress : UploadStatus
        data class Error(val error: LocalDetectionUploader.UploadError) : UploadStatus
        data object Succeed : UploadStatus
    }

    sealed interface ConnectionStatus {
        data object NoAttempts : ConnectionStatus
        data object Connected : ConnectionStatus
        data class Error(val error: LocalDetectionUploader.ConnectionError) : ConnectionStatus
    }
}