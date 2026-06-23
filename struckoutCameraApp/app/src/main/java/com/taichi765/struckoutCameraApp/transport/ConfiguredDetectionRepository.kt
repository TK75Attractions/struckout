package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.proto.Struckout

/**
 * Decide the actual [DetectionRepository] to push detections, based on configurations from [ConfigStoreRepository].
 */
class ConfiguredDetectionRepository(
    private val udpDetectionRepo: UdpDetectionRepository,
    private val diskDetectionRepo: DiskDetectionRepository,
    private val configRepository: ConfigStoreRepository,
) : DetectionRepository {

    override suspend fun pushDetection(packet: Struckout.UdpPacket) {
        if (configRepository.networkFeatureEnabled.value) {
            check(udpDetectionRepo.isBound.value) {
                "UDP must be bound to port before starting detection stream"
            }
            udpDetectionRepo.pushDetection(packet)
        }
        if (configRepository.recordingModeEnabled.value) {
            diskDetectionRepo.pushDetection(packet)
        }
    }
}