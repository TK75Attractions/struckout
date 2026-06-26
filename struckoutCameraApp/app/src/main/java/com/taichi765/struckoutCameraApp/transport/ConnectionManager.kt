package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.di.ApplicationScope
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class ConnectionManager @Inject constructor(
    private val sessionRepository: SessionRepository,
    private val udpDetectionRepository: UdpDetectionRepository,
    private val configRepository: ConfigStoreRepository,
    @ApplicationScope private val scope: CoroutineScope
) {
    fun start() {
        watchTcpConnection()
        watchUdpStatus()
    }

    private fun watchTcpConnection() {
        combine(
            sessionRepository.connState,
            configRepository.networkFeatureEnabled
        ) { connState, networkFeatureEnabled ->
            networkFeatureEnabled &&
                    connState is SessionRepository.ConnectionState.Connected
        }.distinctUntilChanged().onEach { shouldConnect ->
            if (shouldConnect) {
                sessionRepository.connect()
            }
        }.launchIn(scope)
    }

    private fun watchUdpStatus() {
        combine(
            udpDetectionRepository.isBound,
            configRepository.networkFeatureEnabled
        ) { isBoundToPort, networkFeatureEnabled ->
            networkFeatureEnabled && !isBoundToPort
        }.distinctUntilChanged().onEach { shouldBind ->
            if (shouldBind) udpDetectionRepository.bind()
        }.launchIn(scope)
    }
}