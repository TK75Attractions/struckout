package com.taichi765.struckoutCameraApp.config

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.BuildConfig
import com.taichi765.struckoutCameraApp.CaptureSession
import com.taichi765.struckoutCameraApp.camera.DetectionArea
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
    /** Formatted like `--version` of the Rust modules. Only release builds have the commit hash. */
    val version = if (BuildConfig.COMMIT_SHORT_HASH.isEmpty()) {
        "no version"
    } else {
        "${BuildConfig.VERSION_NAME} (${BuildConfig.COMMIT_SHORT_HASH})"
    }

    val uiState = combine(
        configRepository.recordingModeEnabled,
        configRepository.detectionOutputKind,
        networkManager.state,
        configRepository.cameraPose,
        // combine が型つきで受け取れるのは 5 本までなので、解析と撮影の設定をまとめる。
        combine(
            configRepository.detectionArea,
            configRepository.focalLengthRatio,
            configRepository.highSpeedStreamSize,
        ) { area, ratio, size ->
            Triple(area, ratio, size)
        },
    ) { recodingModeEnabled, detectionOutputKind, connectionState, cameraPose, (detectionArea, focalLengthRatio, streamSize) ->
        ConfigUiState(
            recodingModeEnabled,
            detectionOutputKind = detectionOutputKind,
            highSpeedStreamSize = streamSize,
            tcpIsConnected = connectionState.tcpIsConnected(),
            udpIsConnected = connectionState.udpIsConnected(),
            locationX = cameraPose.x.toString(),
            locationY = cameraPose.y.toString(),
            locationZ = cameraPose.z.toString(),
            rotationXDegrees = cameraPose.rotationXDegrees.toString(),
            rotationYDegrees = cameraPose.rotationYDegrees.toString(),
            rotationZDegrees = cameraPose.rotationZDegrees.toString(),
            focalLengthRatio = focalLengthRatio.toString(),
            ignoredLeftPercent = detectionArea.leftPercent.toString(),
            ignoredTopPercent = detectionArea.topPercent.toString(),
            ignoredRightPercent = detectionArea.rightPercent.toString(),
            ignoredBottomPercent = detectionArea.bottomPercent.toString(),
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

        if (newState.highSpeedStreamSize != oldState.highSpeedStreamSize) {
            viewModelScope.launch {
                configRepository.setHighSpeedStreamSize(newState.highSpeedStreamSize)
            }
        }

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

        val newRatio = newState.focalLengthRatio.toString().toDoubleOrNull()
        if (newRatio == null || newRatio < 0.0) {
            // 入力が数でない、または負のとき。前の設定のままにする。
            Timber.tag(TAG).w("ignoring an invalid focal length ratio")
        } else if (newRatio != oldState.focalLengthRatio.toString().toDoubleOrNull()) {
            viewModelScope.launch {
                configRepository.updateFocalLengthRatio(newRatio)
            }
        }

        val newArea = convertCharsToDetectionArea(newState)
        if (newArea == null) {
            // 入力が数でない、または検出を受け付ける場所が残らないとき。前の設定のままにする。
            Timber.tag(TAG).w("ignoring an invalid detection area")
        } else if (newArea != convertCharsToDetectionArea(oldState)) {
            viewModelScope.launch {
                configRepository.updateDetectionArea(newArea)
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

/**
 * @return `null` if [CharSequence]s are not numbers, or the margins leave nothing of the image.
 */
private fun convertCharsToDetectionArea(state: ConfigUiState): DetectionArea? {
    val left = state.ignoredLeftPercent.toString().toDoubleOrNull() ?: return null
    val top = state.ignoredTopPercent.toString().toDoubleOrNull() ?: return null
    val right = state.ignoredRightPercent.toString().toDoubleOrNull() ?: return null
    val bottom = state.ignoredBottomPercent.toString().toDoubleOrNull() ?: return null
    if (!DetectionArea.isValid(left, top, right, bottom)) return null
    return DetectionArea(left, top, right, bottom)
}
