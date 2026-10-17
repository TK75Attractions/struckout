package com.taichi765.struckoutCameraApp.network

import android.os.SystemClock
import com.google.protobuf.ByteString
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.DetectionOutputKind
import com.taichi765.struckoutCameraApp.proto.CameraBallTracker
import com.taichi765.struckoutCameraApp.proto.videoFrame
import com.taichi765.struckoutCameraApp.proto.videoStreamPacket
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import timber.log.Timber
import java.io.BufferedOutputStream
import java.io.DataInputStream
import java.io.IOException
import java.io.OutputStream
import java.net.InetSocketAddress
import java.net.Socket
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.TimeUnit

/**
 * カメラの動画を PC (ball_tracker の 5052 番) に送り、解析を PC に任せる。
 *
 * スマホで解析すると 1 枚 17ms ほどかかり、120fps に追いつかない (KYV45)。PC なら 1 枚 4ms ほどで済む。
 * エンコーダーが出した H.264 をそのまま送るので、スマホはデコードと解析をしなくてよい。
 *
 * 検出の送り先が Network で、ball_tracker からカメラの番号をもらえているあいだ、2 秒おきに PC へつなぎに行く。
 * PC が引き受けているあいだは [active] が true になる。そのあいだ、スマホは自分で解析した検出を送らないこと
 * (PC が同じ形の検出を作って ball_tracker に渡す)。PC につながらないときは、これまでどおりスマホで解析する。
 *
 * [offer] はコーデックのスレッドから呼ぶ。送るのは専用のスレッドで行う。
 *
 * @param configFactory カメラの番号から、PC に送る設定を作る。カメラがまだ動いていないなどで作れないときは null
 * @param requestKeyFrame エンコーダーに、次のフレームをキーフレームにするよう頼む (PC がデコードを始められるように)
 */
class PcVideoStreamer(
    private val networkManager: NetworkManager,
    private val configRepository: ConfigStoreRepository,
    private val configFactory: (cameraId: Int) -> CameraBallTracker.VideoStreamConfig?,
    private val requestKeyFrame: () -> Unit,
) {
    private val _active = MutableStateFlow(false)

    /** PC が解析を引き受けている。 */
    val active: StateFlow<Boolean> = _active.asStateFlow()

    private class Outgoing(
        val data: ByteArray,
        val sensorTimestampNanos: Long,
        val keyFrame: Boolean,
        val codecConfig: Boolean,
    )

    private val queue = ArrayBlockingQueue<Outgoing>(QUEUE_SIZE)

    // 直近の設定 (SPS/PPS)。つないだ直後に送る
    @Volatile
    private var lastCodecConfig: ByteArray? = null

    // PC はキーフレームからしかデコードを始められない。つないだ直後と、詰まって捨てたあとは、次のキーフレームを待つ
    @Volatile
    private var waitingForKeyFrame = true

    @Volatile
    private var running = false
    private var thread: Thread? = null

    // 送っている最中のソケット。止めるときに閉じる (書き込みで止まっているスレッドは割り込みでは起きない)
    @Volatile
    private var socket: Socket? = null

    @Synchronized
    fun start() {
        if (running) return
        running = true
        thread = Thread(::runLoop, "pc-video").apply {
            isDaemon = true
            start()
        }
    }

    @Synchronized
    fun stop() {
        running = false
        thread?.interrupt()
        thread = null
        runCatching { socket?.close() }
        _active.value = false
        queue.clear()
    }

    /** エンコーダーが出したデータを渡す (コーデックのスレッド)。PC につながっていなければ捨てる。 */
    fun offer(data: ByteArray, sensorTimestampNanos: Long, keyFrame: Boolean, codecConfig: Boolean) {
        if (codecConfig) {
            lastCodecConfig = data
        }
        if (!_active.value) return
        if (!codecConfig && waitingForKeyFrame) {
            if (!keyFrame) return
            waitingForKeyFrame = false
        }
        if (!queue.offer(Outgoing(data, sensorTimestampNanos, keyFrame, codecConfig))) {
            // 送るのが追いつかない。途中から送ると PC で壊れた画像になるので、次のキーフレームまで捨てる
            Timber.tag(TAG).w("sending video to PC is falling behind; dropping %d frames", queue.size)
            queue.clear()
            waitingForKeyFrame = true
            requestKeyFrame()
        }
    }

    private fun runLoop() {
        while (running) {
            val config = currentCameraId()?.let(configFactory)
            if (configRepository.detectionOutputKind.value == DetectionOutputKind.NETWORK && config != null) {
                try {
                    stream(config)
                } catch (e: IOException) {
                    Timber.tag(TAG).i("video stream to PC ended: %s", e.toString())
                } catch (e: InterruptedException) {
                    break
                } finally {
                    socket = null
                    _active.value = false
                    queue.clear()
                }
            }
            try {
                Thread.sleep(RETRY_INTERVAL_MILLIS)
            } catch (e: InterruptedException) {
                break
            }
        }
    }

    private fun currentCameraId(): Int? {
        val state = networkManager.currentTcpSession?.state?.value as? SessionState.Connected ?: return null
        return state.cameraID.toInt()
    }

    private fun stream(initialConfig: CameraBallTracker.VideoStreamConfig) {
        Socket().also { this.socket = it }.use { socket ->
            if (!running) return
            socket.connect(InetSocketAddress(REMOTE_ADDRESS, REMOTE_PORT), CONNECT_TIMEOUT_MILLIS)
            socket.tcpNoDelay = true
            // adb reverse は、PC 側で誰も待ち受けていなくても接続をいったん受けるので、返事を待って確かめる
            socket.soTimeout = ANSWER_TIMEOUT_MILLIS
            val input = DataInputStream(socket.getInputStream())
            val length = Integer.reverseBytes(input.readInt())
            val answer = CameraBallTracker.VideoStreamServerPacket.parseFrom(ByteArray(length).also { input.readFully(it) })
            if (!answer.accepted) {
                Timber.tag(TAG).i("PC declined to analyze the video")
                return
            }
            socket.soTimeout = 0
            val output = BufferedOutputStream(socket.getOutputStream(), OUTPUT_BUFFER_BYTES)

            var config = initialConfig
            write(output, videoStreamPacket { this.config = config })
            lastCodecConfig?.let { write(output, framePacket(Outgoing(it, 0, false, true))) }
            output.flush()
            queue.clear()
            waitingForKeyFrame = true
            _active.value = true
            requestKeyFrame()
            Timber.tag(TAG).i("PC accepted the video; analysis moved to PC")

            var lastConfigCheck = SystemClock.elapsedRealtime()
            while (running) {
                val item = queue.poll(POLL_INTERVAL_MILLIS, TimeUnit.MILLISECONDS)
                if (item != null) {
                    write(output, framePacket(item))
                    if (queue.isEmpty()) output.flush()
                }
                val now = SystemClock.elapsedRealtime()
                if (now - lastConfigCheck >= CONFIG_INTERVAL_MILLIS) {
                    lastConfigCheck = now
                    if (configRepository.detectionOutputKind.value != DetectionOutputKind.NETWORK) return
                    val cameraId = currentCameraId() ?: throw IOException("lost the camera id")
                    // ball_tracker につなぎ直して番号が変わったら、新しい番号で送り直す
                    if (cameraId != config.cameraId) throw IOException("the camera id changed")
                    val latest = configFactory(cameraId) ?: continue
                    // 範囲や焦点距離を設定画面で変えたら、送り直す
                    if (latest != config) {
                        config = latest
                        write(output, videoStreamPacket { this.config = latest })
                        output.flush()
                    }
                }
            }
        }
    }

    private fun framePacket(item: Outgoing): CameraBallTracker.VideoStreamPacket = videoStreamPacket {
        frame = videoFrame {
            data = ByteString.copyFrom(item.data)
            codecConfig = item.codecConfig
            keyFrame = item.keyFrame
            sensorTimestampNs = item.sensorTimestampNanos
            if (!item.codecConfig) {
                // アプリが自分で解析するときと同じ、端末の時計での撮影時刻 (UNIX epoch からの ms)
                timestamp = System.currentTimeMillis() - SystemClock.elapsedRealtime() +
                        TimeUnit.NANOSECONDS.toMillis(item.sensorTimestampNanos)
            }
        }
    }

    /** 長さ (u32、リトルエンディアン) と Protobuf。ball_tracker の read_packet と同じ形。 */
    private fun write(output: OutputStream, packet: CameraBallTracker.VideoStreamPacket) {
        val bytes = packet.toByteArray()
        output.write(ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(bytes.size).array())
        output.write(bytes)
    }

    companion object {
        private const val TAG = "PcVideoStreamer"
        private const val REMOTE_ADDRESS = "127.0.0.1"

        /** ball_tracker が動画を受け付けるポート。adb reverse で PC につなぐ */
        const val REMOTE_PORT = 5052

        private const val CONNECT_TIMEOUT_MILLIS = 1000
        private const val ANSWER_TIMEOUT_MILLIS = 2000
        private const val RETRY_INTERVAL_MILLIS = 2000L
        private const val POLL_INTERVAL_MILLIS = 100L
        private const val CONFIG_INTERVAL_MILLIS = 1000L

        /** 送る前にためておけるフレームの数 (120fps で 1 秒) */
        private const val QUEUE_SIZE = 120
        private const val OUTPUT_BUFFER_BYTES = 1 shl 16
    }
}
