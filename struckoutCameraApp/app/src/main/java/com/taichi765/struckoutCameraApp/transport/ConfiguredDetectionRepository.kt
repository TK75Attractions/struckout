package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import javax.inject.Inject

/**
 * Decide the actual [DetectionRepository] to push detections, based on configurations from [ConfigStoreRepository].
 */
class ConfiguredDetectionRepository @Inject constructor(
    private val udpDetectionRepository: UdpDetectionRepository,
    private val diskDetectionRepository: DiskDetectionRepository,
    private val configRepository: ConfigStoreRepository,
) : DetectionRepository {

    override suspend fun pushDetection(data: DetectionData) {
        if (configRepository.networkFeatureEnabled.value) {

            udpDetectionRepository.pushDetection(data)
        }
        if (configRepository.recordingModeEnabled.value) {
            diskDetectionRepository.pushDetection(data)
        }
    }
}