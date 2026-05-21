package com.taichi765.struckoutCameraApp.ble

import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class BleViewModel : ViewModel() {
    private val bleManager = BleManager()

    private var _cameraLocation = MutableStateFlow<CameraLocation?>(null)
    val cameraLocation = _cameraLocation.asStateFlow()

    fun updateCameraLocation(newVal: CameraLocation) {
        _cameraLocation.value = newVal
        viewModelScope.launch {
            bleManager.updateCameraLocation(cameraLocation.value!!)
        }
    }

    init {
        viewModelScope.launch {
            Log.i(TAG, "initializing BleManager")
            bleManager.connect()
            Log.i(TAG, "BleManager initialized successfully")
        }
    }

    companion object {
        const val TAG = "BleViewModel"
    }
}