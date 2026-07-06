package com.taichi765.struckoutCameraApp.config

import com.taichi765.struckoutCameraApp.network.CameraLocationDataSource
import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.flow.StateFlow

interface ConfigStoreRepository : CameraLocationDataSource {
    val recordingModeEnabled: StateFlow<Boolean>
    val detectionOutputKind: StateFlow<DetectionOutputKind>

    suspend fun setDetectionOutputKind(kind: DetectionOutputKind)
    suspend fun toggleRecordingMode()
    suspend fun updateCameraLocation(location: Struckout.CameraLocation)

    companion object {
        const val ENABLE_RECORDING_MODE_DEFAULT = false
    }
}

enum class DetectionOutputKind {
    NETWORK,
    LOCAL,
    NONE
}