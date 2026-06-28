package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.di.ApplicationScope
import com.taichi765.struckoutCameraApp.network.types.ConnectionState
import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.network.types.InstanceState
import com.taichi765.struckoutCameraApp.proto.udpPacket
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
import kotlinx.coroutines.launch
import timber.log.Timber
import javax.inject.Inject
import javax.inject.Singleton

/**
 * [TcpSession]や[UdpConnection]などネットワーク関連のライフサイクルを管理する。
 *
 * 管理されたクラスには直接アクセスするのではなく[NetworkManager]を介してアクセスすべし。
 */
@OptIn(ExperimentalCoroutinesApi::class)
@Singleton
class NetworkManager @Inject constructor(
    private val configRepository: ConfigStoreRepository,
    @ApplicationScope private val applicationScope: CoroutineScope,
    private val tcpSessionFactory: TcpSession.Factory,
    private val udpConnectionFactory: UdpConnection.Factory,
    private val synchronizerFactory: Synchronizer.Factory
) {
    private val _tcpSession = MutableStateFlow<TcpSession?>(null)
    private val _udpConnection = MutableStateFlow<UdpConnection?>(null)
    private val _synchronizer = MutableStateFlow<Synchronizer?>(null)

    val currentTcpSession: TcpSession?
        get() = _tcpSession.value
    val currentUdpConnection: UdpConnection?
        get() = _udpConnection.value
    val currentSynchronizer: Synchronizer?
        get() = _synchronizer.value

    @Suppress("IfThenToElvis")
    private val _tcpState = _tcpSession.flatMapLatest { session ->
        if (session == null) {
            flowOf(InstanceState.NotCreated)
        } else {
            session.state.map {
                InstanceState.Created(it)
            }
        }
    }

    @Suppress("IfThenToElvis")
    private val _udpState = _udpConnection.flatMapLatest { udpConnection ->
        if (udpConnection == null) {
            flowOf(InstanceState.NotCreated)
        } else {
            udpConnection.isConnected.map {
                InstanceState.Created(it)
            }
        }
    }

    @Suppress("IfThenToElvis")
    private val _synchronizerState = _synchronizer.flatMapLatest { synchronizer ->
        if (synchronizer == null) {
            flowOf(InstanceState.NotCreated)
        } else {
            synchronizer.isConnected.map {
                InstanceState.Created(it)
            }
        }
    }

    val state = combine(
        configRepository.networkFeatureEnabled,
        _tcpState,
        _udpState,
        _synchronizerState
    ) { networkFeatureEnabled, tcpState, udpState, synchronizerState ->
        if (networkFeatureEnabled) {
            ConnectionState.NetworkFeatureEnabled(
                tcpInstanceState = tcpState,
                udpInstanceState = udpState,
                synchronizerInstanceState = synchronizerState
            )
        } else {
            ConnectionState.NetworkFeatureDisabled
        }
    }.stateIn(
        scope = applicationScope,
        started = SharingStarted.Eagerly,
        initialValue = ConnectionState.NetworkFeatureDisabled
    )


    fun start() {
        Timber.tag(TAG).i("ConnectionManager started")
        applicationScope.launch {
            watchTcpConnection()
            watchUdpStatus()
            watchSynchronizer()
        }
    }

    suspend fun retryConnection() {
        val session = _tcpSession.value
        check(session != null) {
            "TcpSession instance should be created before retrying connection: state = ${state.value}"
        }
        if (session.state.value is SessionState.Connected) {
            Timber.tag(TAG).w("retryConnection is called when TCP is already connected")
            return
        }
        session.connect()
    }

    private fun CoroutineScope.watchTcpConnection() {
        combine(
            _tcpSession,
            configRepository.networkFeatureEnabled
        ) { tcpSession, networkFeatureEnabled ->
            Pair(
                networkFeatureEnabled &&
                        tcpSession == null,
                networkFeatureEnabled && tcpSession != null && tcpSession.state.value !is SessionState.Connected
            )
        }.distinctUntilChanged().onEach { (shouldCreateInstance, shouldConnect) ->
            if (shouldCreateInstance) {
                _tcpSession.value = tcpSessionFactory.create()
            }
            if (shouldConnect) {
                _tcpSession.value!!.connect()
            }
        }.launchIn(this)
    }

    private fun CoroutineScope.watchUdpStatus() {
        combine(
            _udpConnection,
            configRepository.networkFeatureEnabled
        ) { udpConnection, networkFeatureEnabled ->
            Pair(
                networkFeatureEnabled && udpConnection == null,
                networkFeatureEnabled && udpConnection != null && !udpConnection.isConnected.value
            )
        }.distinctUntilChanged().onEach { (shouldCreateInstance, shouldConnect) ->
            if (shouldCreateInstance) {
                _udpConnection.value = udpConnectionFactory.create()
            }
            if (shouldConnect) {
                _udpConnection.value!!.connect()
            }
        }.launchIn(this)
    }

    private fun CoroutineScope.watchSynchronizer() {
        combine(
            _synchronizer,
            configRepository.networkFeatureEnabled
        ) { synchronizer, networkFeatureEnabled ->
            Pair(
                networkFeatureEnabled && synchronizer == null,
                networkFeatureEnabled && synchronizer != null && !synchronizer.isConnected.value
            )
        }.distinctUntilChanged().onEach { (shouldCreateInstance, shouldConnect) ->
            if (shouldCreateInstance) {
                _synchronizer.value = synchronizerFactory.create()
            }
            if (shouldConnect) {
                _synchronizer.value!!.connect()
            }
        }.launchIn(this)
    }

    /**
     * TODO: 若干責務外かも
     */
    suspend fun pushDetection(data: DetectionData) {
        val session = _tcpSession.value
        check(session != null) {
            "TcpSession instance must be created before sending detection via network"
        }
        val sessionState = session.state.value
        check(sessionState is SessionState.Connected) {
            "TcpSession must be initialized before sending detection via network"
        }
        val udpConnection = _udpConnection.value
        check(udpConnection != null) {
            "UdpConnection instance must be created before sending detection via network"
        }
        check(udpConnection.isConnected.value) {
            "UdpConnection must be initialized before sending detection via network"
        }

        val packet = udpPacket {
            cameraId = sessionState.cameraID.toInt()
            timestamp = data.timestamp
            frameId = data.frameId.toLong()
            data.detections.forEach {
                detectedObjects += it
            }
        }

        udpConnection.sendPacket(packet)
    }


    companion object {
        const val TAG = "ConnectionManager"
    }
}