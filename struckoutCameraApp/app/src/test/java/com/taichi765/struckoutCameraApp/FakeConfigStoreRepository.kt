package com.taichi765.struckoutCameraApp

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.DetectionOutputKind
import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

class FakeConfigStoreRepository(
    initialDetectionOutput: DetectionOutputKind = DetectionOutputKind.NETWORK,
    initialRecordingMode: Boolean = ConfigStoreRepository.ENABLE_RECORDING_MODE_DEFAULT
) : ConfigStoreRepository {
    override val cameraLocation
        get() = throw NotImplementedError("stub!")

    private val _recordingModeEnabled =
        MutableStateFlow(initialRecordingMode)
    override val recordingModeEnabled = _recordingModeEnabled.asStateFlow()

    private val _detectionOutputKind = MutableStateFlow(initialDetectionOutput)
    override val detectionOutputKind = _detectionOutputKind.asStateFlow()

    override suspend fun toggleRecordingMode() {
        _recordingModeEnabled.update { !it }
    }

    override suspend fun setDetectionOutputKind(kind: DetectionOutputKind) {
        _detectionOutputKind.value = kind
    }

    override suspend fun updateCameraLocation(location: Struckout.CameraLocation) {
        throw NotImplementedError("stub!")
    }
}