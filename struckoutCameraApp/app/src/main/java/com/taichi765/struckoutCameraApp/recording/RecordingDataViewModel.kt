package com.taichi765.struckoutCameraApp.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class RecordingDataViewModel @Inject constructor(private val localDetectionRepository: LocalDetectionRepository) :
    ViewModel() {
        
    fun syncLocalDetections() {
        viewModelScope.launch {
            localDetectionRepository.sync()
        }
    }
}