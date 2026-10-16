package com.taichi765.struckoutCameraApp.network

import android.os.Build
import android.os.SystemClock
import com.taichi765.struckoutCameraApp.camera.CameraClockInfo
import com.taichi765.struckoutCameraApp.camera.CameraClockInfoHolder
import com.taichi765.struckoutCameraApp.camera.DetectionArea
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.network.TcpSession.ConnectionError
import com.taichi765.struckoutCameraApp.proto.CameraBallTracker
import com.taichi765.struckoutCameraApp.proto.TcpClientPacketKt
import com.taichi765.struckoutCameraApp.proto.tcpClientPacket
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import timber.log.Timber
import java.io.Closeable
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.net.Socket
import javax.inject.Inject

/**
 * [NetworkManager]がインスタンスのライフサイクルを管理しているので他のところで
 * 直接使うべからず
 *
 * 1 つのインスタンスで接続するのは 1 回だけ。切れたら [state] が [SessionState.DisConnected] に戻るので、
 * [NetworkManager] が新しいインスタンスを作ってつなぎ直す。
 */
class TcpSessionImpl(
    private val configRepository: ConfigStoreRepository,
    cameraClockInfo: CameraClockInfoHolder,
    private val deviceIdentity: DeviceIdentity,
) : TcpSession, SessionStateProvider, Closeable {
    /**
     * [outputActor]や[CameraPoseDataSource]のライフサイクルをApplicationやViewModelではなく
     * 自前で管理したいため
     */
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val _connState =
        MutableStateFlow<InternalSessionState>(InternalSessionState.DisConnected)
    override val state = _connState.map { state ->
        when (state) {
            is InternalSessionState.Connected -> SessionState.Connected(state.cameraID)
            is InternalSessionState.DisConnected -> SessionState.DisConnected
        }
    }.stateIn(
        scope = scope,
        started = SharingStarted.Eagerly,
        initialValue = SessionState.DisConnected
    )

    private val outChannel =
        Channel<OutputAction>(capacity = 4, onBufferOverflow = BufferOverflow.DROP_LATEST)

    /**
     * サーバーに知らせるカメラの設定。最新のものだけ送れば足りる。
     */
    private val cameraInfoChannel = Channel<CameraClockInfo>(Channel.CONFLATED)

    /**
     * 送信は 3 か所 ([outputActor]、[cameraInfoActor]、[inputActor]) から行うので、パケットが混ざらないようにする。
     */
    private val writeMutex = Mutex()


    init {
        combine(configRepository.cameraPose, _connState) { location, connState ->
            if (connState !is InternalSessionState.Connected) {
                Timber.tag(TAG)
                    .d("camera location was updated but cannot sync change to TCP server: TCP is not connected")
                return@combine
            }
            Timber.tag(TAG).d("updating cameraLocation")
            outChannel.send(OutputAction.UpdateCameraLocation(location))
        }.launchIn(scope)
        cameraClockInfo.info.onEach { cameraInfoChannel.send(it) }.launchIn(scope)
    }

    override suspend fun connect(): ConnectionError? {
        return withContext(Dispatchers.IO) {
            Timber.tag(TAG).d("connecting to ball_tracker")
            val socket = try {
                val ret = Socket(TCP_REMOTE_ADDRESS, TCP_REMOTE_PORT)
                // 時刻合わせは往復時間で精度が決まるので、返事をためずにすぐ送る。
                ret.tcpNoDelay = true
                ret
            } catch (e: IOException) {
                Timber.tag(TAG).d("failed to connect to TCP server: $e")
                return@withContext ConnectionError.TcpConnectionFailed(e)
            }

            val packet = try {
                // 先に名乗る。ball_tracker はこれで端末を見分け、いつも同じ camera_id を振る
                writePacket(socket.getOutputStream(), tcpClientPacket {
                    hello = TcpClientPacketKt.hello {
                        deviceId = deviceIdentity.deviceId
                        deviceModel = Build.MODEL
                        appVariant = APP_VARIANT
                    }
                })
                readPacket(socket.getInputStream(), CameraBallTracker.TcpServerPacket::parseFrom)
            } catch (e: IOException) {
                // adb reverse は、PC で ball_tracker が動いていなくても接続をいったん受けるので、ここで分かる
                Timber.tag(TAG).d("ball_tracker did not answer: $e")
                runCatching { socket.close() }
                return@withContext ConnectionError.InitializationFailed(
                    ConnectionError.InitializationError.ReadPacketFailed(
                        e
                    )
                )
            }

            when (packet.dataCase) {
                CameraBallTracker.TcpServerPacket.DataCase.CAMERA_ID -> {
                    val cameraId = packet.cameraId.toUInt()
                    _connState.value = InternalSessionState.Connected(
                        socket,
                        cameraId,
                    )
                    Timber.tag(TAG).i("connected to ball_tracker as camera %d", cameraId.toInt())
                }

                else -> {
                    Timber.tag(TAG)
                        .w("received invalid TCP packet from server")
                    runCatching { socket.close() }
                    return@withContext ConnectionError.InitializationFailed(ConnectionError.InitializationError.InvalidPacket)
                }
            }

            scope.launch {
                outputActor(socket, socket.getOutputStream())
            }
            scope.launch {
                cameraInfoActor(socket, socket.getOutputStream())
            }
            scope.launch {
                inputActor(socket, socket.getInputStream(), socket.getOutputStream())
            }
            return@withContext null
        }
    }

    override fun close() {
        val curState = _connState.value
        if (curState is InternalSessionState.Connected) {
            runCatching { curState.socket.close() }
        } else {
            Timber.tag(TAG).d("TcpSessionRepository.close() is called, but TCP is not connected")
        }
        scope.cancel()
    }

    /**
     * 接続が切れた。[NetworkManager] がつなぎ直す。
     */
    private fun onDisconnected(socket: Socket, cause: Exception) {
        val curState = _connState.value
        if (curState !is InternalSessionState.Connected || curState.socket !== socket) return
        Timber.tag(TAG).i("disconnected from ball_tracker: $cause")
        runCatching { socket.close() }
        _connState.value = InternalSessionState.DisConnected
    }

    /**
     * [outChannel]から[OutputAction]を読んでTCPサーバーに色々送る
     */
    private suspend fun outputActor(socket: Socket, output: OutputStream) {
        try {
            for (action in outChannel) {
                val curState = _connState.value
                if (curState !is InternalSessionState.Connected) continue
                when (action) {
                    is OutputAction.UpdateCameraLocation -> writeMutex.withLock {
                        writePacket(output, tcpClientPacket {
                            this.updateCameraPose = TcpClientPacketKt.updateCameraPose {
                                cameraId = curState.cameraID.toInt()
                                cameraPose = action.location
                            }
                        })
                    }
                }
            }
        } catch (e: IOException) {
            onDisconnected(socket, e)
        }
    }

    /**
     * カメラの設定を、接続した直後と変わるたびにサーバーに知らせる。
     */
    private suspend fun cameraInfoActor(socket: Socket, output: OutputStream) {
        try {
            for (info in cameraInfoChannel) {
                val curState = _connState.value
                if (curState !is InternalSessionState.Connected) continue
                writeMutex.withLock {
                    writePacket(output, tcpClientPacket {
                        this.cameraInfo = TcpClientPacketKt.cameraInfo {
                            cameraId = curState.cameraID.toInt()
                            deviceModel = Build.MODEL
                            appVariant = info.appVariant
                            timestampSource = when (info.timestampSourceRealtime) {
                                true -> CameraBallTracker.TimestampSource.TIMESTAMP_SOURCE_REALTIME
                                false -> CameraBallTracker.TimestampSource.TIMESTAMP_SOURCE_UNKNOWN
                                null -> CameraBallTracker.TimestampSource.TIMESTAMP_SOURCE_UNSPECIFIED
                            }
                            rollingShutterSkewNs = info.rollingShutterSkewNanos
                            exposureTimeNs = info.exposureTimeNanos
                            imageWidth = info.imageWidth
                            imageHeight = info.imageHeight
                            focalLengthPx = info.focalLengthPx
                            frameRate = info.frameRate
                        }
                    })
                }
            }
        } catch (e: IOException) {
            Timber.tag(TAG).w("failed to send camera info: $e")
            onDisconnected(socket, e)
        }
    }

    /**
     * サーバーからの要求に答える。
     *
     * - 時刻合わせ: サーバーは、要求を送ってから返事が届くまでの往復時間と、ここで返す端末の時刻から、端末の時計と
     *   サーバーの時計の差を求める。時刻は端末を再起動するまで単調に進む時計で返す
     *   (壁時計は時刻の修正で飛ぶ)。時刻合わせに対応していないサーバーは要求を送ってこないので、
     *   そのときは何もしない。
     * - カメラの設定: PC が持っている位置と向き、焦点距離、検出の範囲を、この端末の設定に入れる。
     *
     * 読めなくなったら (ball_tracker が止まったら) 接続が切れたとみなす。
     */
    private suspend fun inputActor(socket: Socket, input: InputStream, output: OutputStream) {
        try {
            while (true) {
                val packet = readPacket(input, CameraBallTracker.TcpServerPacket::parseFrom)
                val receivedAt = SystemClock.elapsedRealtimeNanos()
                when (packet.dataCase) {
                    CameraBallTracker.TcpServerPacket.DataCase.TIME_SYNC_REQUEST -> writeMutex.withLock {
                        // 送る直前の時刻を入れたいので、ロックを取ってから読む。
                        writePacket(output, tcpClientPacket {
                            this.timeSyncResponse = TcpClientPacketKt.timeSyncResponse {
                                sequence = packet.timeSyncRequest.sequence
                                receiveElapsedRealtimeNs = receivedAt
                                sendElapsedRealtimeNs = SystemClock.elapsedRealtimeNanos()
                                sendUptimeNs = System.nanoTime()
                            }
                        })
                    }

                    // 設定の保存は時刻合わせの返事を遅らせないよう、別に行う
                    CameraBallTracker.TcpServerPacket.DataCase.CAMERA_SETTINGS ->
                        scope.launch { applySettings(packet.cameraSettings) }

                    else -> Unit
                }
            }
        } catch (e: IOException) {
            onDisconnected(socket, e)
        }
    }

    private suspend fun applySettings(settings: CameraBallTracker.TcpServerPacket.CameraSettings) {
        if (settings.hasCameraPose()) {
            configRepository.updateCameraPose(settings.cameraPose)
        }
        configRepository.updateFocalLengthRatio(settings.focalLengthRatio.coerceAtLeast(0.0))
        if (settings.hasDetectionArea()) {
            val area = settings.detectionArea
            if (DetectionArea.isValid(area.leftPercent, area.topPercent, area.rightPercent, area.bottomPercent)) {
                configRepository.updateDetectionArea(
                    DetectionArea(area.leftPercent, area.topPercent, area.rightPercent, area.bottomPercent)
                )
            } else {
                Timber.tag(TAG).w("ignored an invalid detection area from ball_tracker: $area")
            }
        }
        Timber.tag(TAG).i("applied the camera settings from ball_tracker")
    }

    /**
     * Tells [outChannel] which actions to run.
     */
    private sealed interface OutputAction {
        data class UpdateCameraLocation(val location: CameraBallTracker.CameraPose) :
            OutputAction
    }

    private sealed interface InternalSessionState {
        data class Connected(
            val socket: Socket,
            val cameraID: UInt,
        ) : InternalSessionState

        object DisConnected : InternalSessionState
    }

    class Factory @Inject constructor(
        private val configRepository: ConfigStoreRepository,
        private val cameraClockInfo: CameraClockInfoHolder,
        private val deviceIdentity: DeviceIdentity,
    ) : TcpSession.Factory {
        override fun create(): TcpSession {
            return TcpSessionImpl(configRepository, cameraClockInfo, deviceIdentity)
        }
    }

    companion object {
        const val TAG = "TcpSessionImpl"

        const val TCP_REMOTE_ADDRESS = "127.0.0.1"

        const val TCP_REMOTE_PORT = 6060

        const val APP_VARIANT = "highspeed"
    }
}
