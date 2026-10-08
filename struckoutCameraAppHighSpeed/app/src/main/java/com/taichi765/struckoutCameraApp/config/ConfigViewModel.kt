package com.taichi765.struckoutCameraApp.config

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.CaptureSession
import com.taichi765.struckoutCameraApp.network.NetworkManager
import com.taichi765.struckoutCameraApp.network.types.tcpIsConnected
import com.taichi765.struckoutCameraApp.network.types.udpIsConnected
import com.taichi765.struckoutCameraApp.proto.CameraBallTracker
import com.taichi765.struckoutCameraApp.proto.cameraPose
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import timber.log.Timber
import javax.inject.Inject

@HiltViewModel
class ConfigViewModel @Inject constructor(
    private val networkManager: NetworkManager,
    private val configRepository: ConfigStoreRepository,
    private val captureSession: CaptureSession
) : ViewModel() {
    val uiState = combine(
        configRepository.recordingModeEnabled,
        configRepository.detectionOutputKind,
        networkManager.state,
        configRepository.cameraPose,
    ) { recodingModeEnabled, detectionOutputKind, connectionState, cameraPose ->
        ConfigUiState(
            recodingModeEnabled,
            detectionOutputKind = detectionOutputKind,
            tcpIsConnected = connectionState.tcpIsConnected(),
            udpIsConnected = connectionState.udpIsConnected(),
            locationX = cameraPose.x.toString(),
            locationY = cameraPose.y.toString(),
            locationZ = cameraPose.z.toString(),
            rotationXDegrees = cameraPose.rotationXDegrees.toString(),
            rotationYDegrees = cameraPose.rotationYDegrees.toString(),
            rotationZDegrees = cameraPose.rotationZDegrees.toString(),
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
        networkManager.retryConnection(viewModelScope)
    }

    /**
     * Persists temporary changes.
     */
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
        if (newLocation != convertCharsToCameraLocation(oldState)) {
            viewModelScope.launch {
                configRepository.updateCameraPose(newLocation)
            }
        }
    }

    fun setDetectionOutputKind(kind: DetectionOutputKind) {
        Timber.tag(TAG).d("setting detectionOutputKind to $kind")
        viewModelScope.launch {
            configRepository.setDetectionOutputKind(kind)
        }
    }

    companion object {
        const val TAG = "ConfigViewModel"
    }
}

/**
 * @return `null` if [CharSequence]s contains invalid characters.
 */
private fun convertCharsToCameraLocation(newState: ConfigUiState): CameraBallTracker.CameraPose? {
    val x = runCatching { newState.locationX.toString().toDouble() }.getOrNull() ?: return null
    val y = runCatching { newState.locationY.toString().toDouble() }.getOrNull() ?: return null
    val z = runCatching { newState.locationZ.toString().toDouble() }.getOrNull() ?: return null
    val rotationX =
        runCatching { newState.rotationXDegrees.toString().toDouble() }.getOrNull() ?: return null
    val rotationY =
        runCatching { newState.rotationYDegrees.toString().toDouble() }.getOrNull() ?: return null
    val rotationZ =
        runCatching { newState.rotationZDegrees.toString().toDouble() }.getOrNull() ?: return null
    return cameraPose {
        this.x = x
        this.y = y
        this.z = z
        rotationXDegrees = rotationX
        rotationYDegrees = rotationY
        rotationZDegrees = rotationZ
    }
}


