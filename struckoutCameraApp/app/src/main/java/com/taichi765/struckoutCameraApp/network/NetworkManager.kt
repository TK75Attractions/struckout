package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.DetectionOutputKind
import com.taichi765.struckoutCameraApp.di.ApplicationScope
import com.taichi765.struckoutCameraApp.network.types.ConnectionState
import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.network.types.InstanceState
import com.taichi765.struckoutCameraApp.network.types.TcpState
import com.taichi765.struckoutCameraApp.proto.detectionsPacket
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
import kotlin.uuid.Uuid

/**
 * [TcpSession]や[DataConnection]などネットワーク関連のライフサイクルを管理する。
 *
 * 管理されたクラスには直接アクセスするのではなく[NetworkManager]を介してアクセスすべし。
 */
@OptIn(ExperimentalCoroutinesApi::class)
@Singleton
class NetworkManager @Inject constructor(
    private val configRepository: ConfigStoreRepository,
    @ApplicationScope private val applicationScope: CoroutineScope,
    private val tcpSessionFactory: TcpSession.Factory,
    private val dataConnectionFactory: DataConnection.Factory,
) {
    private val _tcpSession = MutableStateFlow<TcpSession?>(null)
    private val _lastTcpError = MutableStateFlow<TcpSession.ConnectionError?>(null)
    private val _dataConnection = MutableStateFlow<DataConnection?>(null)
    private val _lastDataConnError = MutableStateFlow<DataConnectionError?>(null)

    val currentTcpSession: TcpSession?
        get() = _tcpSession.value
    val currentDataConnection: DataConnection?
        get() = _dataConnection.value

    val tcpState =
        _tcpSession.flatMapLatest { session ->
            if (session == null) {
                flowOf(InstanceState.NotCreated)
            } else {
                combine(session.state, _lastTcpError) { state, error ->
                    InstanceState.Created(
                        TcpState(
                            sessionState = state,
                            lastError = error
                        )
                    )
                }
            }
        }

    @Suppress("IfThenToElvis")
    private val _dataConnState = _dataConnection.flatMapLatest { dataConn ->
        if (dataConn == null) {
            flowOf(InstanceState.NotCreated)
        } else {
            dataConn.isConnected.map {
                InstanceState.Created(it)
            }
        }
    }

    val state = combine(
        configRepository.detectionOutputKind,
        tcpState,
        _dataConnState,
    ) { detectionOutputKind, tcpState, dataConnState ->
        if (detectionOutputKind == DetectionOutputKind.NETWORK) {
            ConnectionState.NetworkOutputEnabled(
                tcpInstanceState = tcpState,
                dataConnInstanceState = dataConnState,
            )
        } else {
            ConnectionState.NetworkOutputDisabled
        }
    }.stateIn(
        scope = applicationScope,
        started = SharingStarted.Eagerly,
        initialValue = ConnectionState.NetworkOutputDisabled
    )

    fun start() {
        Timber.tag(TAG).i("ConnectionManager started")
        applicationScope.launch {
            watchTcpConnection()
            watchDataConnectionStatus()
        }
    }

    private fun CoroutineScope.watchTcpConnection() {
        combine(
            _tcpSession,
            configRepository.detectionOutputKind
        ) { tcpSession, detectionOutputKind ->
            Pair(
                detectionOutputKind == DetectionOutputKind.NETWORK &&
                        tcpSession == null,
                detectionOutputKind == DetectionOutputKind.NETWORK && tcpSession != null && tcpSession.state.value !is SessionState.Connected
            )
        }.distinctUntilChanged().onEach { (shouldCreateInstance, shouldConnect) ->
            if (shouldCreateInstance) {
                _tcpSession.value = tcpSessionFactory.create()
            }
            if (shouldConnect) {
                val error = _tcpSession.value!!.connect()
                if (error != null) {
                    _lastTcpError.value = error
                }
            }
        }.launchIn(this)
    }

    private fun CoroutineScope.watchDataConnectionStatus() {
        combine(
            _dataConnection,
            configRepository.detectionOutputKind
        ) { dataConnection, detectionOutputKind ->
            Pair(
                detectionOutputKind == DetectionOutputKind.NETWORK && dataConnection == null,
                detectionOutputKind == DetectionOutputKind.NETWORK && dataConnection != null && !dataConnection.isConnected.value
            )
        }.distinctUntilChanged().onEach { (shouldCreateInstance, shouldConnect) ->
            if (shouldCreateInstance) {
                _dataConnection.value = dataConnectionFactory.create()
            }
            if (shouldConnect) {
                val error = _dataConnection.value!!.connect()
                if (error != null) {
                    _lastDataConnError.value = error
                }
            }
        }.launchIn(this)
    }

    /**
     * Retries connection for TCP control and TCP data.
     *
     * This function launches coroutine from [scope] and returns immediately.
     * You can see the results via [state].
     */
    fun retryConnection(scope: CoroutineScope) {
        scope.launch {
            retryTcpConnection()
        }
        scope.launch {
            retryDataConnection()
        }
    }

    suspend fun retryTcpConnection() {
        val session = _tcpSession.value ?: run {
            Timber.tag(TAG).d("tcpSession was null, creating new one")
            _tcpSession.value = tcpSessionFactory.create()
            _tcpSession.value!!
        }
        if (session.state.value is SessionState.Connected) {
            Timber.tag(TAG)
                .w("retryTcpConnection(): retryConnection is called when TCP is already connected")
            return
        }
        val error = session.connect()
        if (error != null) {
            _lastTcpError.value = error
        }
    }

    suspend fun retryDataConnection() {
        val dataConn = _dataConnection.value ?: run {
            Timber.tag(TAG).d("retryDataConnection(): dataConnection was null, creating new one")
            _dataConnection.value = dataConnectionFactory.create()
            _dataConnection.value!!
        }
        if (dataConn.isConnected.value) {
            Timber.tag(TAG).w("retryDataConnection() is called but it's already connected")
        }
        val error = dataConn.connect()
        if (error != null) {
            _lastDataConnError.value = error
        }
    }

    /**
     * TODO: 若干責務外かも
     */
    suspend fun pushDetection(data: DetectionData, sessionID: Uuid) {
        val session = _tcpSession.value
        check(session != null) {
            "TcpSession instance must be created before sending detection via network"
        }
        val sessionState = session.state.value
        check(sessionState is SessionState.Connected) {
            "TcpSession must be initialized before sending detection via network"
        }
        val dataConnection = _dataConnection.value
        check(dataConnection != null) {
            "DataConnection instance must be created before sending detection via network"
        }
        check(dataConnection.isConnected.value) {
            "DataConnection must be initialized before sending detection via network"
        }

        val packet = detectionsPacket {
            cameraId = sessionState.cameraID.toInt()
            sessionId = sessionID.toString()
            timestamp = data.timestamp
            frameId = data.frameId.toLong()
            data.detections.forEach {
                detections += it
            }
        }

        dataConnection.sendPacket(packet)
    }


    companion object {
        const val TAG = "ConnectionManager"
    }
}