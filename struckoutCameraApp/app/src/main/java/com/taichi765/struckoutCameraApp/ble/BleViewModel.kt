package com.taichi765.struckoutCameraApp.ble

import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.camera.BleRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class BleViewModel(private val bleRepository: BleRepository) : ViewModel() {

    private var _cameraLocation = MutableStateFlow<CameraLocation?>(null)
    val cameraLocation = _cameraLocation.asStateFlow()

    fun updateCameraLocation(newVal: CameraLocation) {
        Log.i(TAG, "updating camera location")
        _cameraLocation.value = newVal
        viewModelScope.launch {
            bleRepository.sendCameraLocation(cameraLocation.value!!)
        }
    }

    init {
        viewModelScope.launch {
            Log.i(TAG, "initializing BleManager")
            bleRepository.connect()
            Log.i(TAG, "BleManager initialized successfully")
        }
    }

    @Suppress("UNCHECKED_CAST")
    class Factory(private val bleRepository: BleRepository) : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return BleViewModel(bleRepository) as T
        }
    }

    companion object {
        const val TAG = "BleViewModel"
    }
}