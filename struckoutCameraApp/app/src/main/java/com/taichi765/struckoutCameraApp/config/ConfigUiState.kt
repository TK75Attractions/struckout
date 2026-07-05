package com.taichi765.struckoutCameraApp.config

data class ConfigUiState(
    val recodingModeEnabled: Boolean,
    val detectionOutputKind: DetectionOutputKind,
    val udpIsConnected: Boolean,
    val tcpIsConnected: Boolean,
    val locationX: CharSequence,
    val locationY: CharSequence,
    val locationZ: CharSequence,
)

/**
 * Creates [ConfigUiState] with default values.
 */
fun ConfigUiState(): ConfigUiState = ConfigUiState(
    recodingModeEnabled = ConfigStoreRepository.ENABLE_RECORDING_MODE_DEFAULT,
    detectionOutputKind = DetectionOutputKind.NONE,
    udpIsConnected = false,
    tcpIsConnected = false,
    locationX = "dummy!",
    locationY = "dummy!",
    locationZ = "dummy!",
)