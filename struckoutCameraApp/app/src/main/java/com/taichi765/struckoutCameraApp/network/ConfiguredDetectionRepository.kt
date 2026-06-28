package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.recording.LocalDetectionRepository
import timber.log.Timber
import javax.inject.Inject

/**
 * Decide the actual [DetectionRepository] to push detections, based on configurations from [ConfigStoreRepository].
 */
class ConfiguredDetectionRepository @Inject constructor(
    private val networkManager: NetworkManager,
    private val localDetectionRepository: LocalDetectionRepository,
    private val configRepository: ConfigStoreRepository,
) : DetectionRepository {

    override suspend fun pushDetection(data: DetectionData) {
        Timber.tag(TAG).d("networkFeatureEnabled: ${configRepository.networkFeatureEnabled.value}")
        if (configRepository.networkFeatureEnabled.value) {
            networkManager.pushDetection(data)
        }
        if (configRepository.recordingModeEnabled.value) {
            localDetectionRepository.pushDetection(data)
        }
    }

    companion object {
        const val TAG = "ConfiguredDetectionRepository"
    }
}