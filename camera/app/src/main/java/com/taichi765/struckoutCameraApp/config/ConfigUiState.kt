package com.taichi765.struckoutCameraApp.config

data class ConfigUiState(
    val recodingModeEnabled: Boolean,
    val detectionOutputKind: DetectionOutputKind,
    val highSpeedStreamSize: HighSpeedStreamSize,
    val udpIsConnected: Boolean,
    val tcpIsConnected: Boolean,
    val locationX: CharSequence,
    val locationY: CharSequence,
    val locationZ: CharSequence,
    val rotationXDegrees: CharSequence,
    val rotationYDegrees: CharSequence,
    val rotationZDegrees: CharSequence,
    val focalLengthRatio: CharSequence,
    val ignoredLeftPercent: CharSequence,
    val ignoredTopPercent: CharSequence,
    val ignoredRightPercent: CharSequence,
    val ignoredBottomPercent: CharSequence,
)

/**
 * Creates [ConfigUiState] with default values.
 */
fun ConfigUiState(): ConfigUiState = ConfigUiState(
    recodingModeEnabled = ConfigStoreRepository.ENABLE_RECORDING_MODE_DEFAULT,
    detectionOutputKind = DetectionOutputKind.NONE,
    highSpeedStreamSize = HighSpeedStreamSize.HD720,
    udpIsConnected = false,
    tcpIsConnected = false,
    locationX = "0",
    locationY = "0",
    locationZ = "0",
    rotationXDegrees = "0",
    rotationYDegrees = "0",
    rotationZDegrees = "0",
    focalLengthRatio = "0",
    ignoredLeftPercent = "0",
    ignoredTopPercent = "0",
    ignoredRightPercent = "0",
    ignoredBottomPercent = "0",
)
