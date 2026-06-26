package com.taichi765.struckoutCameraApp.config

import com.taichi765.struckoutCameraApp.proto.Struckout

data class ConfigUiState(
    val recodingModeEnabled: Boolean = ConfigStoreRepository.ENABLE_RECORDING_MODE_DEFAULT,
    val networkFeatureEnabled: Boolean = ConfigStoreRepository.ENABLE_NETWORK_FEATURE_DEFAULT,
    val isConnected: Boolean = false,
    val cameraLocation: Struckout.CameraLocation? = null,
    val warningState: WarningState = WarningState()
)