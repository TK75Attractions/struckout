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
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
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
    private val _validationErrors = MutableStateFlow<Map<ConfigField, String>>(emptyMap())
    val validationErrors = _validationErrors.asStateFlow()
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
    fun applyChanges(newState: ConfigUiState): Boolean {
        val oldState = uiState.value

        val validation = validateConfig(newState)
        _validationErrors.value = validation.errors
        if (validation.errors.isNotEmpty()) return false

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

        val newLocation = requireNotNull(validation.cameraPose)
        if (newLocation != validateConfig(oldState).cameraPose) {
            viewModelScope.launch {
                configRepository.updateCameraPose(newLocation)
            }
        }

        val newRatio = requireNotNull(validation.focalLengthRatio)
        if (newRatio != oldState.focalLengthRatio.toString().toDoubleOrNull()) {
            viewModelScope.launch {
                configRepository.updateFocalLengthRatio(newRatio)
            }
        }

        val newArea = requireNotNull(validation.detectionArea)
        if (newArea != convertCharsToDetectionArea(oldState)) {
            viewModelScope.launch {
                configRepository.updateDetectionArea(newArea)
            }
        }

        return true
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

private data class ConfigValidation(
    val errors: Map<ConfigField, String>,
    val cameraPose: CameraBallTracker.CameraPose?,
    val focalLengthRatio: Double?,
    val detectionArea: DetectionArea?,
)

private fun validateConfig(state: ConfigUiState): ConfigValidation {
    val errors = mutableMapOf<ConfigField, String>()
    val values = mutableMapOf<ConfigField, Double>()
    val fields = listOf(
        ConfigField.LOCATION_X to state.locationX,
        ConfigField.LOCATION_Y to state.locationY,
        ConfigField.LOCATION_Z to state.locationZ,
        ConfigField.ROTATION_X_DEGREES to state.rotationXDegrees,
        ConfigField.ROTATION_Y_DEGREES to state.rotationYDegrees,
        ConfigField.ROTATION_Z_DEGREES to state.rotationZDegrees,
        ConfigField.FOCAL_LENGTH_RATIO to state.focalLengthRatio,
        ConfigField.IGNORED_LEFT_PERCENT to state.ignoredLeftPercent,
        ConfigField.IGNORED_TOP_PERCENT to state.ignoredTopPercent,
        ConfigField.IGNORED_RIGHT_PERCENT to state.ignoredRightPercent,
        ConfigField.IGNORED_BOTTOM_PERCENT to state.ignoredBottomPercent,
    )

    fields.forEach { (key, input) ->
        val label = key.label
        val value = input.toString().toDoubleOrNull()
        when {
            input.isBlank() -> errors[key] = "${label}を入力してください。"
            value == null -> errors[key] = "${label}は数値で入力してください。"
            !value.isFinite() -> errors[key] = "${label}には有限の数値を入力してください。"
            else -> values[key] = value
        }
    }

    values[ConfigField.FOCAL_LENGTH_RATIO]?.takeIf { it < 0.0 }?.let {
        errors[ConfigField.FOCAL_LENGTH_RATIO] = "焦点距離比は 0 以上で入力してください。"
    }

    val left = values[ConfigField.IGNORED_LEFT_PERCENT]
    val top = values[ConfigField.IGNORED_TOP_PERCENT]
    val right = values[ConfigField.IGNORED_RIGHT_PERCENT]
    val bottom = values[ConfigField.IGNORED_BOTTOM_PERCENT]
    if (left != null && right != null && left >= 0.0 && right >= 0.0 && left + right >= 100.0) {
        errors[ConfigField.IGNORED_LEFT_PERCENT] = "左右の検出無視幅の合計は 100% 未満にしてください。"
        errors[ConfigField.IGNORED_RIGHT_PERCENT] = "左右の検出無視幅の合計は 100% 未満にしてください。"
    }
    if (top != null && bottom != null && top >= 0.0 && bottom >= 0.0 && top + bottom >= 100.0) {
        errors[ConfigField.IGNORED_TOP_PERCENT] = "上下の検出無視幅の合計は 100% 未満にしてください。"
        errors[ConfigField.IGNORED_BOTTOM_PERCENT] = "上下の検出無視幅の合計は 100% 未満にしてください。"
    }
    listOf(
        ConfigField.IGNORED_LEFT_PERCENT to left,
        ConfigField.IGNORED_TOP_PERCENT to top,
        ConfigField.IGNORED_RIGHT_PERCENT to right,
        ConfigField.IGNORED_BOTTOM_PERCENT to bottom,
    ).forEach { (key, value) ->
        if (value != null && value < 0.0) errors[key] = "検出無視幅は 0 以上で入力してください。"
    }

    val poseKeys = listOf(
        ConfigField.LOCATION_X, ConfigField.LOCATION_Y, ConfigField.LOCATION_Z,
        ConfigField.ROTATION_X_DEGREES, ConfigField.ROTATION_Y_DEGREES, ConfigField.ROTATION_Z_DEGREES,
    )
    val pose = if (poseKeys.all { it in values && it !in errors }) cameraPose {
        x = values.getValue(ConfigField.LOCATION_X)
        y = values.getValue(ConfigField.LOCATION_Y)
        z = values.getValue(ConfigField.LOCATION_Z)
        rotationXDegrees = values.getValue(ConfigField.ROTATION_X_DEGREES)
        rotationYDegrees = values.getValue(ConfigField.ROTATION_Y_DEGREES)
        rotationZDegrees = values.getValue(ConfigField.ROTATION_Z_DEGREES)
    } else null
    val ratio = values[ConfigField.FOCAL_LENGTH_RATIO]?.takeIf { ConfigField.FOCAL_LENGTH_RATIO !in errors }
    val areaKeys = listOf(
        ConfigField.IGNORED_LEFT_PERCENT, ConfigField.IGNORED_TOP_PERCENT, ConfigField.IGNORED_RIGHT_PERCENT, ConfigField.IGNORED_BOTTOM_PERCENT,
    )
    val area = if (areaKeys.all { it in values && it !in errors }) {
        DetectionArea(
            requireNotNull(left),
            requireNotNull(top),
            requireNotNull(right),
            requireNotNull(bottom),
        )
    } else null
    return ConfigValidation(errors, pose, ratio, area)
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
