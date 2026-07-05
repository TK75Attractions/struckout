package com.taichi765.struckoutCameraApp.config

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.CaptureSession
import com.taichi765.struckoutCameraApp.network.NetworkManager
import com.taichi765.struckoutCameraApp.network.TcpSession
import com.taichi765.struckoutCameraApp.network.types.tcpIsConnected
import com.taichi765.struckoutCameraApp.network.types.udpIsConnected
import com.taichi765.struckoutCameraApp.proto.Struckout
import com.taichi765.struckoutCameraApp.proto.cameraLocation
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class ConfigViewModel @Inject constructor(
    private val networkManager: NetworkManager,
    private val configRepository: ConfigStoreRepository,
    private val captureSession: CaptureSession
) : ViewModel() {
    /**
     * TODO: [TcpSession]に持たせる
     */
    private val _cameraLocation = MutableStateFlow<Struckout.CameraLocation?>(null)

    /**
     * invariant: `isConnected` should be always `false` if `networkFeature` is disabled.
     *
     * TODO: isConnectedとnetworkFeatureEnabledをsealed interfaceにする
     */
    val uiState = combine(
        configRepository.recordingModeEnabled,
        configRepository.detectionOutputKind,
        networkManager.state,
        _cameraLocation,
    ) { recodingModeEnabled, detectionOutputKind, connectionState, cameraLocation ->
        ConfigUiState(
            recodingModeEnabled,
            detectionOutputKind = detectionOutputKind,
            tcpIsConnected = connectionState.tcpIsConnected(),
            udpIsConnected = connectionState.udpIsConnected(),
            locationX = (cameraLocation?.x ?: 0).toString(),
            locationY = (cameraLocation?.y ?: 0).toString(),
            locationZ = (cameraLocation?.z ?: 0).toString(),
        )
    }.stateIn(
        viewModelScope,
        started = SharingStarted.Eagerly,
        initialValue = ConfigUiState()
    )

    fun resetSession() {
        captureSession.reset()
    }

    fun retryConnection() {
        viewModelScope.launch {
            networkManager.retryConnection()
        }
    }

    fun applyChanges(newState: ConfigUiState) {
        val oldState = uiState.value

        if (newState.recodingModeEnabled != oldState.recodingModeEnabled) {
            viewModelScope.launch {
                configRepository.toggleRecordingMode()
            }
        }

        val newKind = newState.detectionOutputKind
        if (newKind != oldState.detectionOutputKind) {
            viewModelScope.launch {
                configRepository.setDetectionOutputKind(newKind)
            }
        }

        val newLocation =
            convertCharsToCameraLocation(newState) ?: TODO("あとでUI追加する")
        if (newLocation != _cameraLocation.value) {
            viewModelScope.launch {
                configRepository.updateCameraLocation(newLocation)
            }
        }
    }


    fun toggleRecordingMode() {
        viewModelScope.launch {
            configRepository.toggleRecordingMode()
        }
    }

    companion object {
        const val TAG = "ConfigViewModel"
    }
}

/**
 * @return `null` if [CharSequence]s contains invalid characters.
 */
private fun convertCharsToCameraLocation(newState: ConfigUiState): Struckout.CameraLocation? {
    val x = runCatching { newState.locationX.toString().toDouble() }.getOrNull() ?: return null
    val y = runCatching { newState.locationY.toString().toDouble() }.getOrNull() ?: return null
    val z = runCatching { newState.locationZ.toString().toDouble() }.getOrNull() ?: return null
    return cameraLocation {
        this.x = x
        this.y = y
        this.z = z
    }
}


