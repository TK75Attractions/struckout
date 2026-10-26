package com.taichi765.struckoutCameraApp.config

import com.taichi765.struckoutCameraApp.camera.DetectionArea
import com.taichi765.struckoutCameraApp.network.CameraPoseDataSource
import com.taichi765.struckoutCameraApp.proto.CameraBallTracker
import kotlinx.coroutines.flow.StateFlow

interface ConfigStoreRepository : CameraPoseDataSource {
    val recordingModeEnabled: StateFlow<Boolean>
    val detectionOutputKind: StateFlow<DetectionOutputKind>
    val detectionArea: StateFlow<DetectionArea>
    val highSpeedStreamSize: StateFlow<HighSpeedStreamSize>

    /**
     * 焦点距離を、解析画像の幅に対する比で指定する。0 のときは端末が返す値から計算する。
     * 端末が返す値が実際の画角と合わないときに、`calibrate_intrinsics.py` で求めた値を入れる。
     */
    val focalLengthRatio: StateFlow<Double>

    suspend fun setDetectionOutputKind(kind: DetectionOutputKind)
    suspend fun toggleRecordingMode()
    suspend fun updateCameraPose(pose: CameraBallTracker.CameraPose)
    suspend fun updateDetectionArea(area: DetectionArea)
    suspend fun updateFocalLengthRatio(ratio: Double)
    suspend fun setHighSpeedStreamSize(size: HighSpeedStreamSize)

    companion object {
        const val ENABLE_RECORDING_MODE_DEFAULT = false
    }
}

enum class DetectionOutputKind {
    NETWORK,
    LOCAL,
    NONE
}
