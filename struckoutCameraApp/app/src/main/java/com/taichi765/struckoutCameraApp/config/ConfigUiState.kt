package com.taichi765.struckoutCameraApp.config

import com.taichi765.struckoutCameraApp.camera.types.CameraLocation

data class ConfigUiState(
    val recodingModeEnabled: Boolean = ConfigStoreRepository.ENABLE_RECORDING_MODE_DEFAULT,
    val networkFeatureEnabled: Boolean = ConfigStoreRepository.ENABLE_NETWORK_FEATURE_DEFAULT,
    val isConnected: Boolean = false,
    val cameraLocation: CameraLocation? = null
)