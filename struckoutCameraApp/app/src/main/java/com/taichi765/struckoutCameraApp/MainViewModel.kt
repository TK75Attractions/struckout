package com.taichi765.struckoutCameraApp

import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.ble.BleManager
import kotlinx.coroutines.launch

class MainViewModel : ViewModel() {
    val bleManager = BleManager()


    init {
        Log.i(TAG, "initializing MainViewModel")
        viewModelScope.launch {
            bleManager.connect()
        }
    }


    companion object {
        const val TAG = "MainViewModel"
    }
}