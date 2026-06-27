package com.taichi765.struckoutCameraApp.config

import com.taichi765.struckoutCameraApp.proto.Struckout

data class ConfigUiState(
    val recodingModeEnabled: Boolean = ConfigStoreRepository.ENABLE_RECORDING_MODE_DEFAULT,
    val networkFeatureEnabled: Boolean = ConfigStoreRepository.ENABLE_NETWORK_FEATURE_DEFAULT,
    val udpIsConnected: Boolean = false,
    val tcpIsConnected: Boolean = false,
    val cameraLocation: Struckout.CameraLocation? = null,
    val warningState: WarningState = WarningState()
)

/**
 * `true`のとき警告を表示する
 */
data class WarningState(
    val showX: Boolean = false,
    val showY: Boolean = false,
    val showZ: Boolean = false
)

fun WarningState.isAllOk(): Boolean {
    if (showX) return false
    if (showY) return false
    if (showZ) return false
    return true
}