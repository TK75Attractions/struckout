package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach

@Singleton
class ConnectionManager @Inject constructor(
    private val sessionRepository: SessionRepository,
    private val udpDetectionRepository: UdpDetectionRepository,
    private val configRepository: ConfigStoreRepository,
    private val applicationScope: CoroutineScope
) {
    fun start() {
        watchTcpConnection()
        watchUdpStatus()
    }

    private fun watchTcpConnection() {
        combine(
            sessionRepository.state,
            configRepository.networkFeatureEnabled
        ) { connState, networkFeatureEnabled ->
            networkFeatureEnabled &&
                    connState is ConnectionState.Disconnected
        }.distinctUntilChanged().onEach { shouldConnect ->
            if (shouldConnect) {
                sessionRepository.connect()
            }
        }.launchIn(applicationScope)
    }

    private fun watchUdpStatus() {
        combine(
            udpDetectionRepository.isBound,
            configRepository.networkFeatureEnabled
        ) { isBoundToPort, networkFeatureEnabled ->
            networkFeatureEnabled && !isBoundToPort
        }.distinctUntilChanged().onEach { shouldBind ->
            if (shouldBind) udpDetectionRepository.bind()
        }.launchIn(applicationScope)
    }
}