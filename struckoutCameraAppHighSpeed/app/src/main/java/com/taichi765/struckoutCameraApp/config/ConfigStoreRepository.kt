package com.taichi765.struckoutCameraApp.config

import com.taichi765.struckoutCameraApp.network.CameraPoseDataSource
import com.taichi765.struckoutCameraApp.proto.CameraBallTracker
import kotlinx.coroutines.flow.StateFlow

interface ConfigStoreRepository : CameraPoseDataSource {
    val recordingModeEnabled: StateFlow<Boolean>
    val detectionOutputKind: StateFlow<DetectionOutputKind>

    suspend fun setDetectionOutputKind(kind: DetectionOutputKind)
    suspend fun toggleRecordingMode()
    suspend fun updateCameraPose(pose: CameraBallTracker.CameraPose)

    companion object {
        const val ENABLE_RECORDING_MODE_DEFAULT = false
    }
}

enum class DetectionOutputKind {
    NETWORK,
    LOCAL,
    NONE
}