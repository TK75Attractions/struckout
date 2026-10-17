package com.taichi765.struckoutCameraApp.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.network.LocalDetectionUploader
import com.taichi765.struckoutCameraApp.network.LocalDetectionUploader.ConnectionError
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import timber.log.Timber
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

    /**
     * Will be reset when [LocalDetectionUploader.connect] has succeeded.
     */
    private val _lastConnectionError =
        MutableStateFlow<ConnectionError?>(null)

    val connectionStatus = combine(
        localDetectionUploader.isConnected,
        _lastConnectionError
    ) { isConnected, lastConnectionError ->
        if (isConnected) {
            ConnectionStatus.Connected
        } else if (lastConnectionError == null) {
            ConnectionStatus.NoAttempts
        } else {
            ConnectionStatus.Error(lastConnectionError)
        }
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Eagerly,
        initialValue = ConnectionStatus.NoAttempts
    )

    private val _uploadStatus = MutableStateFlow<UploadStatus>(UploadStatus.NotStarted)
    val uploadStatus = _uploadStatus.asStateFlow()

    private val _showConfirmDeleteDialog = MutableStateFlow(false)
    val showConfirmDeleteDialog = _showConfirmDeleteDialog.asStateFlow()


    init {
        connectionStatus.filterIsInstance<ConnectionStatus.NoAttempts>().onEach {
            connect()
        }.launchIn(viewModelScope)
    }

    fun uploadLocalDetections() {
        viewModelScope.launch {
            _uploadStatus.value = UploadStatus.InProgress
            val frames = localDetectionRepository.loadAll()
            val error = localDetectionUploader.upload(frames)
            if (error == null) {
                Timber.tag(TAG).i("succeeded to upload local data")
                _uploadStatus.value = UploadStatus.Succeed
                _showConfirmDeleteDialog.value = true
            } else {
                Timber.tag(TAG).w("failed to upload local data: $error")
                // TODO: エラーの種類に応じてログとかやる
                _uploadStatus.value = UploadStatus.Error(error)
            }
        }
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
                Timber.tag(TAG).w("failed to connect to xtask-sync server: $error")
                _lastConnectionError.value = error
            } else {
                Timber.tag(TAG).i("succeeded to connect to xtask-sync server")
                _lastConnectionError.value = null
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
        /**
         * When [connectionStatus] is set to [ConnectionStatus.NoAttempts], [LocalDataViewModel] automatically
         * tries to connect [LocalDetectionUploader].
         */
        data object NoAttempts : ConnectionStatus
        data object Connected : ConnectionStatus
        data class Error(val error: ConnectionError) : ConnectionStatus
    }

    companion object {
        const val TAG = "LocalDataViewModel"
    }
}