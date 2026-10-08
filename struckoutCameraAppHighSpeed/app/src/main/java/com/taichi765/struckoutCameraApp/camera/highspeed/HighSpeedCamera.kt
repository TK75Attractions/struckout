package com.taichi765.struckoutCameraApp.camera.highspeed

import android.annotation.SuppressLint
import android.content.Context
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraConstrainedHighSpeedCaptureSession
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CameraMetadata
import android.hardware.camera2.CaptureRequest
import android.hardware.camera2.params.OutputConfiguration
import android.hardware.camera2.params.SessionConfiguration
import android.os.Handler
import android.os.HandlerThread
import android.os.SystemClock
import android.util.Range
import android.util.Size
import android.view.Surface
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.opencv.core.CvType
import org.opencv.core.Mat
import timber.log.Timber
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.Executor
import java.util.concurrent.TimeUnit
import javax.inject.Inject
import kotlin.math.roundToInt

/**
 * 背面カメラを Camera2 の高速撮影セッション (constrained high speed) で動かし、
 * 縮小したフレームを解析用に渡す。
 *
 * 通常のセッションでは 30fps が上限の端末でも、このセッションなら 120fps で撮れる。
 * 出力先に YUV の ImageReader を使えないため、[GlFrameReader] 経由でフレームを取り出す。
 *
 * スレッドは 3 本使う。
 * - カメラ: Camera2 のコールバック
 * - GL: フレームの取り込みと縮小・読み出し
 * - 解析: [FrameListener] の呼び出し
 *
 * 解析が追いつかないときは、読み出しを飛ばしてフレームを捨てる (カメラは止めない)。
 */
class HighSpeedCamera @Inject constructor(@ApplicationContext private val context: Context) {
    fun interface FrameListener {
        /**
         * 解析スレッドで呼ばれる。
         *
         * @param rgba センサーの向きのままの RGBA 画像。呼び出しが返ると再利用されるので、
         * 保持する場合はコピーすること
         * @param timestampNanos フレームのセンサー時刻
         */
        fun onFrame(rgba: Mat, timestampNanos: Long)
    }

    /**
     * 直近 1 秒ほどの実績。
     *
     * @param cameraFps センサー時刻の間隔から求めた、カメラが実際に出している fps
     * @param analyzedFps 解析まで終えた fps
     * @param droppedFrames 解析が追いつかず捨てたフレーム数
     * @param readbackMillis 縮小と読み出しにかかった時間の平均
     * @param analysisMillis [FrameListener.onFrame] にかかった時間の平均
     */
    data class Stats(
        val cameraFps: Double,
        val analyzedFps: Double,
        val droppedFrames: Int,
        val readbackMillis: Double,
        val analysisMillis: Double,
    )

    sealed interface State {
        data object Stopped : State
        data object Starting : State

        /**
         * @param streamSize カメラが出力している大きさ
         * @param analysisSize 解析に渡している大きさ
         */
        data class Running(val streamSize: Size, val analysisSize: Size, val fps: Int) : State
        data class Failed(val message: String) : State
    }

    private val _state = MutableStateFlow<State>(State.Stopped)
    val state = _state.asStateFlow()

    private val _stats = MutableStateFlow<Stats?>(null)
    val stats = _stats.asStateFlow()

    private var run: Run? = null

    @Synchronized
    fun start(listener: FrameListener) {
        if (run != null) return
        _state.value = State.Starting
        _stats.value = null
        run = Run(listener).also { it.start() }
    }

    @Synchronized
    fun stop() {
        run?.stop()
        run = null
        _state.value = State.Stopped
    }

    private class Frame(val buffer: ByteBuffer, val timestampNanos: Long)

    /**
     * start から stop までの 1 回分。スレッドや GL の資源をまとめて持ち、まとめて捨てる。
     */
    private inner class Run(private val listener: FrameListener) {
        private val cameraThread = HandlerThread("hs-camera").apply { start() }
        private val cameraHandler = Handler(cameraThread.looper)
        private val glThread = HandlerThread("hs-gl").apply { start() }
        private val glHandler = Handler(glThread.looper)

        // 空きバッファと、解析待ちのフレーム。空きがなければそのフレームは捨てる。
        private val freeBuffers = ArrayBlockingQueue<ByteBuffer>(BUFFER_COUNT)
        private val pendingFrames = ArrayBlockingQueue<Frame>(BUFFER_COUNT)

        @Volatile
        private var stopped = false

        // GL スレッドからのみ触る
        private var reader: GlFrameReader? = null
        private var surface: Surface? = null

        // カメラスレッドからのみ触る
        private var device: CameraDevice? = null
        private var session: CameraCaptureSession? = null

        // 集計用。GL スレッドからのみ触る (解析側の 2 つを除く)
        private var windowStartMillis = 0L
        private var acquiredFrames = 0
        private var droppedFrames = 0
        private var firstTimestampNanos = 0L
        private var lastTimestampNanos = 0L
        private var readbackNanos = 0L
        private var readFrames = 0

        @Volatile
        private var analyzedFrames = 0

        @Volatile
        private var analysisNanos = 0L

        fun start() {
            glHandler.post {
                if (stopped) return@post
                try {
                    val manager =
                        context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
                    val mode = selectMode(manager)
                    val analysisSize = Size(
                        ANALYSIS_WIDTH,
                        // 縦横比を保つ。偶数にそろえる。
                        (ANALYSIS_WIDTH.toDouble() * mode.size.height / mode.size.width / 2)
                            .roundToInt() * 2
                    )

                    val reader = GlFrameReader.create(analysisSize.width, analysisSize.height)
                    this.reader = reader
                    repeat(BUFFER_COUNT) {
                        freeBuffers.add(
                            ByteBuffer.allocateDirect(analysisSize.width * analysisSize.height * 4)
                                .order(ByteOrder.nativeOrder())
                        )
                    }
                    reader.surfaceTexture.setDefaultBufferSize(mode.size.width, mode.size.height)
                    reader.surfaceTexture.setOnFrameAvailableListener({ onFrameAvailable() }, glHandler)
                    val surface = Surface(reader.surfaceTexture)
                    this.surface = surface

                    Thread({ analysisLoop(analysisSize) }, "hs-analysis")
                        .apply { isDaemon = true }
                        .start()
                    cameraHandler.post { openCamera(manager, mode, surface, analysisSize) }
                } catch (e: Exception) {
                    fail("failed to prepare the high speed camera", e)
                }
            }
        }

        fun stop() {
            stopped = true
            cameraHandler.post {
                runCatching { session?.close() }
                runCatching { device?.close() }
                session = null
                device = null
                cameraThread.quitSafely()

                // カメラを閉じてから出力先を捨てる。
                glHandler.post {
                    surface?.release()
                    runCatching { reader?.release() }
                    surface = null
                    reader = null
                    glThread.quitSafely()
                }
            }
        }

        @SuppressLint("MissingPermission") // カメラ画面に入る前に App が権限を取っている
        private fun openCamera(
            manager: CameraManager,
            mode: Mode,
            surface: Surface,
            analysisSize: Size
        ) {
            if (stopped) return
            try {
                manager.openCamera(mode.cameraId, object : CameraDevice.StateCallback() {
                    override fun onOpened(camera: CameraDevice) {
                        if (stopped) {
                            camera.close()
                            return
                        }
                        device = camera
                        createSession(camera, mode, surface, analysisSize)
                    }

                    override fun onDisconnected(camera: CameraDevice) {
                        camera.close()
                        device = null
                        fail("camera was disconnected", null)
                    }

                    override fun onError(camera: CameraDevice, error: Int) {
                        camera.close()
                        device = null
                        fail("camera device error: $error", null)
                    }
                }, cameraHandler)
            } catch (e: Exception) {
                fail("failed to open the camera", e)
            }
        }

        private fun createSession(
            camera: CameraDevice,
            mode: Mode,
            surface: Surface,
            analysisSize: Size
        ) {
            val executor = Executor { cameraHandler.post(it) }
            val callback = object : CameraCaptureSession.StateCallback() {
                override fun onConfigured(configured: CameraCaptureSession) {
                    if (stopped) {
                        configured.close()
                        return
                    }
                    session = configured
                    try {
                        val highSpeed = configured as CameraConstrainedHighSpeedCaptureSession
                        val request = camera.createCaptureRequest(CameraDevice.TEMPLATE_RECORD).apply {
                            addTarget(surface)
                            set(CaptureRequest.CONTROL_AE_TARGET_FPS_RANGE, mode.fpsRange)
                            // 電子式の手ぶれ補正は画像を切り出してずらすので、位置の計算が狂う。
                            set(
                                CaptureRequest.CONTROL_VIDEO_STABILIZATION_MODE,
                                CameraMetadata.CONTROL_VIDEO_STABILIZATION_MODE_OFF
                            )
                        }.build()
                        highSpeed.setRepeatingBurst(
                            highSpeed.createHighSpeedRequestList(request), null, cameraHandler
                        )
                        Timber.tag(TAG)
                            .i("high speed session started: stream=${mode.size} fps=${mode.fpsRange} analysis=$analysisSize")
                        _state.value =
                            State.Running(mode.size, analysisSize, mode.fpsRange.upper)
                    } catch (e: Exception) {
                        fail("failed to start the high speed capture", e)
                    }
                }

                override fun onConfigureFailed(failed: CameraCaptureSession) {
                    fail("failed to configure the high speed session (stream=${mode.size})", null)
                }
            }
            try {
                camera.createCaptureSession(
                    SessionConfiguration(
                        SessionConfiguration.SESSION_HIGH_SPEED,
                        listOf(OutputConfiguration(surface)),
                        executor,
                        callback
                    )
                )
            } catch (e: Exception) {
                fail("failed to create the high speed session", e)
            }
        }

        // GL スレッド
        private fun onFrameAvailable() {
            val reader = reader ?: return
            if (stopped) return
            try {
                val timestamp = reader.acquireFrame()
                val now = SystemClock.elapsedRealtime()
                if (acquiredFrames == 0) {
                    windowStartMillis = now
                    firstTimestampNanos = timestamp
                }
                acquiredFrames++
                lastTimestampNanos = timestamp

                val buffer = freeBuffers.poll()
                if (buffer == null) {
                    droppedFrames++
                } else {
                    val begin = System.nanoTime()
                    reader.readFrame(buffer)
                    readbackNanos += System.nanoTime() - begin
                    readFrames++
                    pendingFrames.add(Frame(buffer, timestamp))
                }

                if (now - windowStartMillis >= STATS_INTERVAL_MILLIS) {
                    publishStats(now)
                }
            } catch (e: Exception) {
                fail("failed to read a frame", e)
            }
        }

        private fun publishStats(now: Long) {
            val seconds = (now - windowStartMillis) / 1000.0
            val analyzed = analyzedFrames
            val stats = Stats(
                cameraFps = if (acquiredFrames > 1) {
                    (acquiredFrames - 1) * 1e9 / (lastTimestampNanos - firstTimestampNanos)
                } else 0.0,
                analyzedFps = analyzed / seconds,
                droppedFrames = droppedFrames,
                readbackMillis = if (readFrames > 0) readbackNanos / 1e6 / readFrames else 0.0,
                analysisMillis = if (analyzed > 0) analysisNanos / 1e6 / analyzed else 0.0,
            )
            _stats.value = stats
            Timber.tag(TAG).i(
                "camera=%.1ffps analyzed=%.1ffps dropped=%d readback=%.2fms analysis=%.2fms",
                stats.cameraFps, stats.analyzedFps, stats.droppedFrames,
                stats.readbackMillis, stats.analysisMillis
            )

            acquiredFrames = 0
            droppedFrames = 0
            readbackNanos = 0
            readFrames = 0
            // 解析スレッドとの間で数フレーム分ずれることがあるが、目安の表示なので許容する。
            analyzedFrames = 0
            analysisNanos = 0
        }

        private fun analysisLoop(size: Size) {
            try {
                while (!stopped) {
                    // stop() を見落とさないよう、待ちっぱなしにはしない。
                    val frame = pendingFrames.poll(100, TimeUnit.MILLISECONDS) ?: continue
                    val mat = Mat(size.height, size.width, CvType.CV_8UC4, frame.buffer)
                    val begin = System.nanoTime()
                    try {
                        listener.onFrame(mat, frame.timestampNanos)
                    } finally {
                        mat.release()
                        analysisNanos += System.nanoTime() - begin
                        analyzedFrames++
                        freeBuffers.add(frame.buffer)
                    }
                }
            } catch (e: Exception) {
                fail("frame analysis failed", e)
            }
        }

        private fun fail(message: String, cause: Throwable?) {
            if (stopped) return
            Timber.tag(TAG).e(cause, message)
            _state.value = State.Failed(if (cause != null) "$message: $cause" else message)
        }
    }

    private data class Mode(val cameraId: String, val size: Size, val fpsRange: Range<Int>)

    /**
     * 背面カメラの高速撮影の設定から、[TARGET_FPS] 固定で撮れるものを選ぶ。
     */
    private fun selectMode(manager: CameraManager): Mode {
        // CameraRepository と同じく、最初の背面カメラを使う。
        val cameraId = manager.cameraIdList.firstOrNull { id ->
            manager.getCameraCharacteristics(id)
                .get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_BACK
        } ?: throw IllegalStateException("This device does not have a back camera.")

        val characteristics = manager.getCameraCharacteristics(cameraId)
        val capabilities =
            characteristics.get(CameraCharacteristics.REQUEST_AVAILABLE_CAPABILITIES) ?: intArrayOf()
        check(CameraMetadata.REQUEST_AVAILABLE_CAPABILITIES_CONSTRAINED_HIGH_SPEED_VIDEO in capabilities) {
            "This device does not support constrained high speed video."
        }
        val map = characteristics.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP)
            ?: throw IllegalStateException("This device does not report stream configurations.")

        val candidates = map.highSpeedVideoSizes.mapNotNull { size ->
            val ranges = map.getHighSpeedVideoFpsRangesFor(size)
            Timber.tag(TAG).i("high speed config: $size ${ranges.joinToString()}")
            // 下限と上限が同じ範囲を選ぶ。下限が 30 の範囲だと、プレビュー用の出力は 30fps になる。
            ranges.firstOrNull { it.lower == TARGET_FPS && it.upper == TARGET_FPS }
                ?.let { Mode(cameraId, size, it) }
        }
        check(candidates.isNotEmpty()) { "This device does not support ${TARGET_FPS}fps capture." }

        // 縮小して使うので大きい画像は要らない。小さいほうが GPU の負荷が軽い。
        // ただし 4:3 のセンサーから横いっぱいに切り出される 16:9 を優先する (内部パラメータの換算の前提)。
        return candidates
            .filter { it.size.width >= ANALYSIS_WIDTH }
            .sortedWith(
                compareBy<Mode> { if (it.size.width * 9 == it.size.height * 16) 0 else 1 }
                    .thenBy { it.size.width * it.size.height }
            )
            .firstOrNull()
            ?: candidates.maxBy { it.size.width * it.size.height }
    }

    companion object {
        const val TAG = "HighSpeedCamera"
        const val TARGET_FPS = 120

        /** 解析に渡す画像の幅。高さはカメラの出力の縦横比から決まる。 */
        const val ANALYSIS_WIDTH = 640

        private const val BUFFER_COUNT = 3
        private const val STATS_INTERVAL_MILLIS = 1000L
    }
}
