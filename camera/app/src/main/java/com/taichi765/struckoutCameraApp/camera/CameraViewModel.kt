package com.taichi765.struckoutCameraApp.camera

import android.os.SystemClock
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.core.graphics.createBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.CaptureSession
import com.taichi765.struckoutCameraApp.camera.highspeed.HighSpeedCamera
import com.taichi765.struckoutCameraApp.camera.types.FrameID
import com.taichi765.struckoutCameraApp.camera.types.increment
import com.taichi765.struckoutCameraApp.camera.types.toULong
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.PushFrameUseCase
import com.taichi765.struckoutCameraApp.network.NetworkManager
import com.taichi765.struckoutCameraApp.network.PcVideoStreamer
import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.proto.CameraBallTracker
import com.taichi765.struckoutCameraApp.proto.detection
import com.taichi765.struckoutCameraApp.proto.videoStreamConfig
import com.taichi765.struckoutCameraApp.remote.RemoteCommands
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.opencv.android.Utils
import org.opencv.core.Core
import org.opencv.core.Mat
import org.opencv.core.Point
import org.opencv.core.Rect
import org.opencv.core.Scalar
import org.opencv.imgproc.Imgproc
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import javax.inject.Inject

@HiltViewModel
class CameraViewModel @Inject constructor(
    private val pushFrame: PushFrameUseCase,
    private val cameraRepository: CameraRepository,
    private val configRepository: ConfigStoreRepository,
    private val camera: HighSpeedCamera,
    private val clockInfo: CameraClockInfoHolder,
    private val frameSaver: CalibrationFrameSaver,
    networkManager: NetworkManager,
    private val captureSession: CaptureSession,
    private val remoteCommands: RemoteCommands,
) : ViewModel() {
    private val _contoursImage = MutableStateFlow<ImageBitmap?>(null)
    val contoursImage = _contoursImage.asStateFlow()

    init {
        clockInfo.update {
            it.copy(appVariant = "highspeed", frameRate = HighSpeedCamera.TARGET_FPS.toDouble())
        }
    }

    private val saveFrameRequested = AtomicBoolean(false)
    private val _savedFrame = MutableStateFlow<String?>(null)

    /**
     * 最後に保存した較正用の画像の場所 (または失敗の理由)。
     */
    val savedFrame = _savedFrame.asStateFlow()

    /**
     * 次のフレームを、較正用の画像として保存する。
     */
    fun requestSaveFrame() {
        saveFrameRequested.set(true)
    }

    /**
     * @param rgb 検出の枠を描き込む前の解析画像。この呼び出しの間だけ有効
     */
    private fun saveFrameIfRequested(rgb: Mat) {
        if (!saveFrameRequested.compareAndSet(true, false)) {
            return
        }
        val copy = rgb.clone()
        val intrinsics = cameraRepository.intrinsicsFor(copy.cols(), copy.rows())
        viewModelScope.launch(Dispatchers.IO) {
            _savedFrame.value =
                frameSaver.save(copy, intrinsics, cameraRepository.sensorOrientation)
            copy.release()
        }
    }

    val cameraState = camera.state
    val cameraStats = camera.stats

    // ball_tracker が動画を受け付けるなら、解析は PC に任せる (この端末の解析は 120fps に追いつかない)
    private val pcStreamer = PcVideoStreamer(
        networkManager = networkManager,
        configRepository = configRepository,
        configFactory = ::buildVideoStreamConfig,
        requestKeyFrame = { camera.requestKeyFrame() },
    )

    /** PC が解析を引き受けている。そのあいだ、この端末は解析せず、検出も送らない。 */
    val analyzedOnPc = pcStreamer.active

    init {
        // PC が解析しているあいだは、この端末のデコードをキーフレームだけにして、エンコーダーに回路を空ける
        viewModelScope.launch {
            pcStreamer.active.collect { camera.setKeyFramePreview(it) }
        }
        // PC から adb で「Save frame」を頼まれたとき (scripts/camera_remote.py)
        viewModelScope.launch {
            remoteCommands.saveFrame.collect { requestSaveFrame() }
        }
    }

    // 以下は解析スレッドからのみ触る
    private var frameId = FrameID(0u)
    private val rgb = Mat()
    private var lastPreviewNanos = 0L
    private var cameraSettingsJob: Job? = null

    fun startCamera() {
        if (cameraSettingsJob != null) return
        cameraSettingsJob = viewModelScope.launch {
            configRepository.highSpeedStreamSize.collect { size ->
                // 動画の寸法と解析画像の寸法を PC に送り直すため、送信もカメラと一緒に開き直す。
                pcStreamer.stop()
                camera.stop()
                pcStreamer.start()
                camera.start(::onFrame, pcStreamer::offer, size)
            }
        }
        remoteCommands.cameraScreenActive = true
    }

    fun stopCamera() {
        cameraSettingsJob?.cancel()
        cameraSettingsJob = null
        remoteCommands.cameraScreenActive = false
        pcStreamer.stop()
        camera.stop()
    }

    override fun onCleared() {
        remoteCommands.cameraScreenActive = false
        pcStreamer.stop()
        camera.stop()
    }

    /**
     * PC に送る設定。アプリが自分で解析するときと同じ値 (解析画像の大きさ、内部パラメータ、範囲、解析の設定) にする。
     * カメラが動いていないときは null。
     */
    private fun buildVideoStreamConfig(cameraId: Int): CameraBallTracker.VideoStreamConfig? {
        val running = camera.state.value as? HighSpeedCamera.State.Running ?: return null
        val analysis = running.analysisSize
        val intrinsics = cameraRepository.intrinsicsFor(analysis.width, analysis.height)
        val area = configRepository.detectionArea.value
        val tracker = cameraRepository.tracker
        return videoStreamConfig {
            this.cameraId = cameraId
            sessionId = captureSession.sessionId.toString()
            mimeType = "video/avc"
            videoWidth = running.streamSize.width
            videoHeight = running.streamSize.height
            frameRate = running.fps.toDouble()
            analysisWidth = analysis.width
            analysisHeight = analysis.height
            fx = intrinsics.fx
            fy = intrinsics.fy
            cx = intrinsics.cx
            cy = intrinsics.cy
            deviceToCamera += cameraRepository.deviceToCameraRotation.toList()
            ignoredLeftPercent = area.leftPercent
            ignoredTopPercent = area.topPercent
            ignoredRightPercent = area.rightPercent
            ignoredBottomPercent = area.bottomPercent
            accumulateWeight = tracker.accumulateWeight
            binaryThreshold = tracker.binaryThreshold
            minContourArea = tracker.minContourArea
            foregroundPersistenceFrames = tracker.foregroundPersistenceFrames
            minColorShift = tracker.minColorShift
        }
    }

    /**
     * @param rgba センサーの向きのままの画像。この呼び出しの間だけ有効
     */
    private fun onFrame(rgba: Mat, timestampNanos: Long) {
        // 設定の変更がすぐ反映されるよう、範囲はフレームごとに読む。
        val area = configRepository.detectionArea.value
        if (pcStreamer.active.value) {
            // 解析は PC が行う。画面と較正用の画像のためだけに使う
            Imgproc.cvtColor(rgba, rgb, Imgproc.COLOR_RGBA2RGB)
            saveFrameIfRequested(rgb)
            updatePreview(rgba, emptyList(), emptyList(), area)
            return
        }
        // 背景の更新 (accumulateWeighted) が 4 チャンネルを扱えないので、アルファを落として渡す。
        Imgproc.cvtColor(rgba, rgb, Imgproc.COLOR_RGBA2RGB)
        val (bboxes, ignored) = cameraRepository.tracker.nextFrame(rgb).partition {
            area.contains(it.x + it.width / 2.0, it.y + it.height / 2.0, rgb.cols(), rgb.rows())
        }

        saveFrameIfRequested(rgb)
        updatePreview(rgba, bboxes, ignored, area)
        if (bboxes.isEmpty()) {
            return
        }

        frameId = frameId.increment()

        // create and send packet
        val curFrameID = frameId.toULong()
        val data = DetectionData(
            timestamp = getTimestamp(TimeUnit.NANOSECONDS.toMillis(timestampNanos)),
            sensorTimestampNanos = timestampNanos,
            frameId = curFrameID,
            detections = bboxes.map { bbox ->
                val worldDirection =
                    cameraRepository.calc(bbox, rgba.cols(), rgba.rows())
                detection {
                    layX = worldDirection.x
                    layY = worldDirection.y
                    layZ = worldDirection.z
                    bboxWidth = bbox.width
                    bboxHeight = bbox.height
                    // 行ごとの露光の時刻差を補正するのに、画像のどの行に写ったかが要る。
                    imageX = (bbox.x + bbox.width / 2f) / rgba.cols()
                    imageY = (bbox.y + bbox.height / 2f) / rgba.rows()
                }
            })
        viewModelScope.launch {
            pushFrame(data)
        }
    }

    /**
     * 画面表示用の画像を更新する。毎フレーム作ると解析が 120fps に追いつかないので間引く。
     * 検出があったフレームを優先し、検出がなくてもカメラの向きを合わせられるよう時々更新する。
     */
    private fun updatePreview(
        rgba: Mat,
        bboxes: List<Rect>,
        ignored: List<Rect>,
        area: DetectionArea
    ) {
        val now = System.nanoTime()
        val interval =
            if (bboxes.isEmpty() && ignored.isEmpty()) IDLE_PREVIEW_INTERVAL_NANOS else PREVIEW_INTERVAL_NANOS
        if (now - lastPreviewNanos < interval) {
            return
        }
        lastPreviewNanos = now

        // 検出は RGB に写した側で済んでいるので、ここで描き込んでも結果には影響しない。
        // 範囲を合わせやすいよう、範囲の枠と、範囲の外で捨てた検出も描く。
        if (!area.coversWholeImage) {
            val bounds = area.inAnalysisImage(rgba.cols(), rgba.rows())
            Imgproc.rectangle(
                rgba,
                Point(bounds.left, bounds.top),
                Point(bounds.right, bounds.bottom),
                Scalar(0.0, 255.0, 0.0, 255.0)
            )
        }
        ignored.forEach {
            Imgproc.rectangle(rgba, it, Scalar(128.0, 128.0, 128.0, 255.0))
        }
        bboxes.forEach {
            Imgproc.rectangle(rgba, it, Scalar(255.0, 0.0, 0.0, 255.0))
        }
        val rotated = Mat()
        Core.rotate(rgba, rotated, Core.ROTATE_90_CLOCKWISE)
        val bitmap = createBitmap(rotated.cols(), rotated.rows())
        Utils.matToBitmap(rotated, bitmap)
        rotated.release()
        _contoursImage.value = bitmap.asImageBitmap()
    }

    fun flashVideo() {
        //videoEncoder.stop()
    }

    companion object {
        const val TAG = "CameraViewModel"
        private val PREVIEW_INTERVAL_NANOS = TimeUnit.MILLISECONDS.toNanos(100)
        private val IDLE_PREVIEW_INTERVAL_NANOS = TimeUnit.MILLISECONDS.toNanos(500)
    }
}

/**
 * @return elapsed time from UNIX epoch to image's timestamp
 */
private fun getTimestamp(imageTimestamp: Long): Long {
    // TODO: 平均取って精度上げてもいいかも
    val boot = System.currentTimeMillis() - SystemClock.elapsedRealtime()
    return boot + imageTimestamp
}
