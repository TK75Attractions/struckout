package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepositoryImpl
import com.taichi765.struckoutCameraApp.di.ApplicationScope
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import timber.log.Timber
import javax.inject.Inject
import javax.inject.Singleton

/**
 * [TcpSession]や[UdpDetectionRepository]などネットワーク関連のライフサイクルを管理する。
 *
 * 管理されたクラスには直接アクセスするのではなく[ConnectionManager]を介してアクセスすべし。
 */
@OptIn(ExperimentalCoroutinesApi::class)
@Singleton
class ConnectionManager @Inject constructor(
    private val configRepository: ConfigStoreRepositoryImpl,
    private val cameraLocationDataSource: CameraLocationDataSource,
    @ApplicationScope private val scope: CoroutineScope,
) {
    private val tcpSession = MutableStateFlow<TcpSession?>(null)
    private val udpDetectionRepository = MutableStateFlow<UdpDetectionRepository?>(null)

    @Suppress("IfThenToElvis")
    private val tcpState = tcpSession.flatMapLatest { session ->
        if (session == null) {
            flowOf(InstanceState.NotCreated)
        } else {
            session.state.map {
                InstanceState.Created(it)
            }
        }
    }

    @Suppress("IfThenToElvis")
    private val udpState = udpDetectionRepository.flatMapLatest { udpConnection ->
        if (udpConnection == null) {
            flowOf(InstanceState.NotCreated)
        } else {
            udpConnection.isBound.map {
                InstanceState.Created(it)
            }
        }
    }

    val state = combine(
        configRepository.networkFeatureEnabled,
        tcpState,
        udpState
    ) { networkFeatureEnabled, tcpState, udpState ->
        if (networkFeatureEnabled) {
            ConnectionState.NetworkFeatureEnabled(
                tcpState, udpState
            )
        } else {
            ConnectionState.NetworkFeatureDisabled
        }
    }.stateIn(
        scope = scope,
        started = SharingStarted.Eagerly,
        initialValue = ConnectionState.NetworkFeatureDisabled
    )

    fun start() {
        Timber.tag(TAG).i("ConnectionManager started")
        watchTcpConnection()
        watchUdpStatus()
    }

    private fun watchTcpConnection() {
        combine(
            tcpSession,
            configRepository.networkFeatureEnabled
        ) { tcpSession, networkFeatureEnabled ->
            Pair(
                networkFeatureEnabled &&
                        tcpSession == null,
                networkFeatureEnabled && tcpSession != null && tcpSession.state.value !is SessionState.Connected
            )
        }.distinctUntilChanged().onEach { (shouldCreateInstance, shouldConnect) ->
            if (shouldCreateInstance) {
                tcpSession.value = TcpSession(cameraLocationDataSource)
            }
            if (shouldConnect) {
                tcpSession.value!!.connect()
            }
        }.launchIn(scope)
    }

    private fun watchUdpStatus() {
        combine(
            udpDetectionRepository,
            configRepository.networkFeatureEnabled
        ) { udpConnection, networkFeatureEnabled ->
            Pair(
                networkFeatureEnabled && udpConnection == null,
                networkFeatureEnabled && udpConnection != null && !udpConnection.isBound.value
            )
        }.distinctUntilChanged().onEach { (shouldCreateInstance, shouldBind) ->
            if (shouldCreateInstance) {
                val tcpSession = tcpSession.value
                check(tcpSession != null) {
                    "TcpSession instance should be created before creating UdpConnection instance"
                }
                udpDetectionRepository.value = UdpDetectionRepository(tcpSession)
            }
            if (shouldBind) {
                udpDetectionRepository.value!!.bind()
            }
        }.launchIn(scope)
    }

    companion object {
        const val TAG = "ConnectionManager"
    }
}