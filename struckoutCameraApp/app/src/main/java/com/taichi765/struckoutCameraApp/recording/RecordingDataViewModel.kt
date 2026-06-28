package com.taichi765.struckoutCameraApp.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.network.NetworkManager
import com.taichi765.struckoutCameraApp.network.types.synchronizerIsConnected
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
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

    fun syncLocalDetections() {
        check(networkManager.state.value.synchronizerIsConnected())
        viewModelScope.launch {
            val out = networkManager.currentSynchronizer!!.getOutputStream()
            val input = networkManager.currentSynchronizer!!.getInputStream()
            syncInProgress.value = true
            localDetectionRepository.syncAll(out, input)
        }
        syncInProgress.value = false
    }
}