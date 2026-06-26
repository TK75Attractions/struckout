package com.taichi765.struckoutCameraApp.config

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.proto.Struckout
import com.taichi765.struckoutCameraApp.transport.SessionRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import timber.log.Timber
import javax.inject.Inject

@HiltViewModel
class ConfigViewModel @Inject constructor(
    private val sessionRepository: SessionRepository,
    private val configRepository: ConfigStoreRepository
) : ViewModel() {
    /**
     * TODO: [SessionRepository]に持たせる
     */
    private val _cameraLocation = MutableStateFlow<Struckout.CameraLocation?>(null)

    /**
     * invariant: `isConnected` should be always `false` if `networkFeature` is disabled.
     *
     * TODO: isConnectedとnetworkFeatureEnabledをsealed interfaceにする
     */
    val uiState = combine(
        configRepository.recordingModeEnabled,
        configRepository.networkFeatureEnabled,
        _cameraLocation,
        sessionRepository.connState
    ) { recodingModeEnabled, networkFeatureEnabled, cameraLocation, connState ->
        ConfigUiState(
            recodingModeEnabled,
            networkFeatureEnabled,
            connState is SessionRepository.ConnectionState.Connected,
            cameraLocation,
        )
    }.stateIn(
        viewModelScope,
        started = SharingStarted.Eagerly,
        initialValue = ConfigUiState()
    )

    fun updateCameraLocation(value: Struckout.CameraLocation) {
        Timber.tag(TAG).i("updating camera location")

        viewModelScope.launch {
            sessionRepository.updateCameraLocation(value)
        }
    }

    fun connect() {
        viewModelScope.launch {
            sessionRepository.connect()
        }
    }

    fun close() {
        viewModelScope.launch {
            sessionRepository.close()
        }
    }

    fun toggleRecordingMode() {
        viewModelScope.launch {
            configRepository.toggleRecodingMode()
        }
    }

    fun toggleNetworkFeature() {
        viewModelScope.launch {
            configRepository.toggleNetworkFeature()
        }
    }

    fun disableNetworkFeature() {
        check(configRepository.networkFeatureEnabled.value) {
            "disableNetworkFeature should not be called when it's already disabled"
        }
        viewModelScope.launch {
            configRepository.disableNetworkFeature()
        }
    }

    companion object {
        const val TAG = "ConfigViewModel"
    }
}


