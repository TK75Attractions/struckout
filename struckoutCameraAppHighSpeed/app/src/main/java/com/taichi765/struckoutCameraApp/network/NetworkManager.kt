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
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
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
    private val tcpMutex = Mutex()
    private val dataMutex = Mutex()

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
        applicationScope.launch { superviseTcpSession() }
        applicationScope.launch { superviseDataConnection() }
    }

    /**
     * 検出の送り先が Network のあいだ、ball_tracker とのつながりを保つ。切れたり、ball_tracker がまだ動いて
     * いなかったりしたら、[RETRY_INTERVAL_MILLIS] おきにつなぎ直す。PC 側をいつ起動・再起動してもよい。
     */
    private suspend fun superviseTcpSession() {
        while (true) {
            if (configRepository.detectionOutputKind.value == DetectionOutputKind.NETWORK) {
                retryTcpConnection()
            }
            delay(RETRY_INTERVAL_MILLIS)
        }
    }

    private suspend fun superviseDataConnection() {
        while (true) {
            if (configRepository.detectionOutputKind.value == DetectionOutputKind.NETWORK) {
                retryDataConnection()
            }
            delay(RETRY_INTERVAL_MILLIS)
        }
    }

    /**
     * Retries connection for TCP control and TCP data.
     *
     * This function launches coroutine from [scope] and returns immediately.
     * You can see the results via [state].
     */
    fun retryConnection(scope: CoroutineScope) {
        Timber.tag(TAG).d("retrying connection...")
        scope.launch {
            retryTcpConnection()
        }
        scope.launch {
            retryDataConnection()
        }
    }

    /**
     * つながっていなければ、新しい [TcpSession] でつなぎ直す ([TcpSession] は 1 回しかつなげない)。
     */
    suspend fun retryTcpConnection() = tcpMutex.withLock {
        val current = _tcpSession.value
        if (current != null && current.state.value is SessionState.Connected) return@withLock
        current?.close()
        val session = tcpSessionFactory.create()
        _tcpSession.value = session
        val error = session.connect()
        if (error != null) {
            if (_lastTcpError.value == null) {
                Timber.tag(TAG).i("cannot reach ball_tracker; retrying every %d ms", RETRY_INTERVAL_MILLIS)
            }
        } else {
            Timber.tag(TAG).i("connected to ball_tracker")
        }
        _lastTcpError.value = error
    }

    suspend fun retryDataConnection() = dataMutex.withLock {
        val dataConn = _dataConnection.value ?: dataConnectionFactory.create().also {
            _dataConnection.value = it
        }
        if (dataConn.isConnected.value) return@withLock
        _lastDataConnError.value = dataConn.connect()
    }

    /**
     * TODO: 若干責務外かも
     */
    suspend fun pushDetection(data: DetectionData, sessionID: Uuid) {
        // つながっていないあいだの検出は捨てる (つなぎ直すのは superviseTcpSession と superviseDataConnection)
        val sessionState = _tcpSession.value?.state?.value as? SessionState.Connected ?: return
        val dataConnection = _dataConnection.value?.takeIf { it.isConnected.value } ?: return

        val packet = detectionsPacket {
            cameraId = sessionState.cameraID.toInt()
            sessionId = sessionID.toString()
            timestamp = data.timestamp
            sensorTimestampNs = data.sensorTimestampNanos
            frameId = data.frameId.toLong()
            data.detections.forEach {
                detections += it
            }
        }

        dataConnection.sendPacket(packet)
    }


    companion object {
        const val TAG = "ConnectionManager"

        /** ball_tracker につながらないとき、つなぎ直す間隔 */
        const val RETRY_INTERVAL_MILLIS = 2000L
    }
}