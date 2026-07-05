package com.taichi765.struckoutCameraApp.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.network.NetworkManager
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class RecordingDataViewModel @Inject constructor(
    private val localDetectionRepository: LocalDetectionRepository,
    private val networkManager: NetworkManager
) : ViewModel() {
    val rowCount = localDetectionRepository.rowCount.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(5000),
        initialValue = 0
    )

    val syncInProgress = MutableStateFlow(false)

    private val _showConfirmDeleteDialog = MutableStateFlow(false)
    val showConfirmDeleteDialog = _showConfirmDeleteDialog.asStateFlow()

    fun syncLocalDetections() {
        check(networkManager.state.value.synchronizerIsConnected())
        viewModelScope.launch {
            val out = networkManager.currentLocalDetectionUploader!!.getOutputStream()
            val input = networkManager.currentLocalDetectionUploader!!.getInputStream()
            syncInProgress.value = true
            localDetectionRepository.syncAll(out, input)
        }
        syncInProgress.value = false

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
}