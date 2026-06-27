package com.taichi765.struckoutCameraApp

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

class FakeConfigStoreRepository(
    initialNetworkFeatureEnabled: Boolean = ConfigStoreRepository.ENABLE_NETWORK_FEATURE_DEFAULT,
    initialRecordingMode: Boolean = ConfigStoreRepository.ENABLE_RECORDING_MODE_DEFAULT
) : ConfigStoreRepository {
    override val cameraLocation
        get() = throw NotImplementedError("stub!")
    private val _networkFeatureEnabled =
        MutableStateFlow(initialNetworkFeatureEnabled)
    override val networkFeatureEnabled = _networkFeatureEnabled.asStateFlow()

    private val _recordingModeEnabled =
        MutableStateFlow(initialRecordingMode)
    override val recordingModeEnabled = _recordingModeEnabled.asStateFlow()

    override suspend fun disableNetworkFeature() {
        _networkFeatureEnabled.value = false
    }

    override suspend fun toggleNetworkFeature() {
        _networkFeatureEnabled.update { !it }
    }

    override suspend fun toggleRecordingMode() {
        _recordingModeEnabled.update { !it }
    }

    override suspend fun updateCameraLocation(location: Struckout.CameraLocation) {
        throw NotImplementedError("stub!")
    }
}