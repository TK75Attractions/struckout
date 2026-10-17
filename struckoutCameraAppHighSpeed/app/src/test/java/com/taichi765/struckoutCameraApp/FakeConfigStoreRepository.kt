package com.taichi765.struckoutCameraApp

import com.taichi765.struckoutCameraApp.camera.DetectionArea
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.DetectionOutputKind
import com.taichi765.struckoutCameraApp.proto.CameraBallTracker
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

class FakeConfigStoreRepository(
    initialDetectionOutput: DetectionOutputKind = DetectionOutputKind.NETWORK,
    initialRecordingMode: Boolean = ConfigStoreRepository.ENABLE_RECORDING_MODE_DEFAULT
) : ConfigStoreRepository {
    override val cameraPose
        get() = throw NotImplementedError("stub!")

    private val _recordingModeEnabled =
        MutableStateFlow(initialRecordingMode)
    override val recordingModeEnabled = _recordingModeEnabled.asStateFlow()

    private val _detectionOutputKind = MutableStateFlow(initialDetectionOutput)
    override val detectionOutputKind = _detectionOutputKind.asStateFlow()

    private val _detectionArea = MutableStateFlow(DetectionArea())
    override val detectionArea = _detectionArea.asStateFlow()

    private val _focalLengthRatio = MutableStateFlow(0.0)
    override val focalLengthRatio = _focalLengthRatio.asStateFlow()

    override suspend fun toggleRecordingMode() {
        _recordingModeEnabled.update { !it }
    }

    override suspend fun setDetectionOutputKind(kind: DetectionOutputKind) {
        _detectionOutputKind.value = kind
    }

    override suspend fun updateCameraPose(pose: CameraBallTracker.CameraPose) {
        throw NotImplementedError("stub!")
    }

    override suspend fun updateDetectionArea(area: DetectionArea) {
        _detectionArea.value = area
    }

    override suspend fun updateFocalLengthRatio(ratio: Double) {
        _focalLengthRatio.value = ratio
    }
}