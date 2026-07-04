package com.taichi765.struckoutCameraApp.config

import com.taichi765.struckoutCameraApp.CaptureSession
import com.taichi765.struckoutCameraApp.network.NetworkManager
import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.recording.LocalDetectionRepository
import javax.inject.Inject

/**
 * Decide the actual destination to push detections, based on configurations from [ConfigStoreRepository].
 */
class PushFrameUseCase @Inject constructor(
    private val networkManager: NetworkManager,
    private val localDetectionRepository: LocalDetectionRepository,
    private val configRepository: ConfigStoreRepository,
    private val captureSession: CaptureSession
) {
    suspend operator fun invoke(data: DetectionData) {
        if (configRepository.networkFeatureEnabled.value) {
            networkManager.pushDetection(data, captureSession.sessionId)
        }
        if (configRepository.recordingModeEnabled.value) {
            localDetectionRepository.pushDetection(data, captureSession.sessionId)
        }
    }
}