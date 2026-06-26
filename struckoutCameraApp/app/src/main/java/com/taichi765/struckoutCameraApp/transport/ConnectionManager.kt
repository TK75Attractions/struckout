package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.di.ApplicationScope
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import timber.log.Timber
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class ConnectionManager @Inject constructor(
    private val tcpSessionRepository: TcpSessionRepository,
    private val udpDetectionRepository: UdpDetectionRepository,
    private val configRepository: ConfigStoreRepository,
    @ApplicationScope private val scope: CoroutineScope
) {
    fun start() {
        Timber.tag(TAG).i("ConnectionManager started")
        watchTcpConnection()
        watchUdpStatus()
    }

    private fun watchTcpConnection() {
        combine(
            tcpSessionRepository.state,
            configRepository.networkFeatureEnabled
        ) { connState, networkFeatureEnabled ->
            networkFeatureEnabled &&
                    connState !is SessionState.Connected
        }.distinctUntilChanged().onEach { shouldConnect ->
            if (shouldConnect) {
                tcpSessionRepository.connect()
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

    companion object {
        const val TAG = "ConnectionManager"
    }
}