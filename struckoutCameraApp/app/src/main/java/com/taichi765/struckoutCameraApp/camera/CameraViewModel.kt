package com.taichi765.struckoutCameraApp.camera

import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CaptureRequest
import android.hardware.camera2.TotalCaptureResult
import android.os.SystemClock
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.core.graphics.createBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.camera.types.FrameID
import com.taichi765.struckoutCameraApp.camera.types.increment
import com.taichi765.struckoutCameraApp.camera.types.toULong
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.PushFrameUseCase
import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.proto.detection
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.opencv.android.Utils
import org.opencv.core.Mat
import org.opencv.imgproc.Imgproc
import java.util.concurrent.atomic.AtomicBoolean
import javax.inject.Inject

@HiltViewModel
class CameraViewModel @Inject constructor(
    private val pushFrame: PushFrameUseCase,
    private val cameraRepository: CameraRepository,
    private val configRepository: ConfigStoreRepository,
    private val clockInfo: CameraClockInfoHolder,
    private val frameSaver: CalibrationFrameSaver,
    //private val videoEncoderFactory: VideoEncoder.Factory,
) : ViewModel() {
    private val _contoursImage = MutableStateFlow<ImageBitmap?>(null)
    val contoursImage = _contoursImage.asStateFlow()

    init {
        // フレームレートは端末まかせで、実際の値はここでは分からない。
        clockInfo.update { it.copy(appVariant = "camerax", frameRate = 30.0) }
    }

    /**
     * 行ごとの露光の時刻差と露光時間は撮影結果からしか分からないので、撮影のたびに受け取る。
     */
    val captureCallback = object : CameraCaptureSession.CaptureCallback() {
        override fun onCaptureCompleted(
            session: CameraCaptureSession,
            request: CaptureRequest,
            result: TotalCaptureResult
        ) {
            clockInfo.onCaptureResult(result)
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

    private var frameId = FrameID(0u)

    //private lateinit var videoEncoder: VideoEncoder

    val analyzer = MyAnalyzer(
        cameraRepository.tracker,
        { configRepository.detectionArea.value },
        ::saveFrameIfRequested
    ) { result ->
        val bitmap = createBitmap(result.mat.cols(), result.mat.rows())
        Utils.matToBitmap(result.mat, bitmap)
        val imageBitMap = bitmap.asImageBitmap()

        /*if (!::videoEncoder.isInitialized) {
            videoEncoder = videoEncoderFactory.create(imageBitMap.width, imageBitMap.height)
            viewModelScope.launch {
                videoEncoder.run()
            }
        }*/
        if (result.bboxes.count() == 0) {
            return@MyAnalyzer
        }

        // update properties
        _contoursImage.value = imageBitMap
        frameId = frameId.increment()

        // create and send packet
        val curFrameID = frameId.toULong()
        val data = DetectionData(
            timestamp = getTimestamp(result.imageTimestampMillis),
            sensorTimestampNanos = result.imageTimestampNanos,
            frameId = curFrameID,
            detections = result.bboxes.map { bbox ->
                val worldDirection =
                    cameraRepository.calc(bbox, result.imageWidth, result.imageHeight)
                detection {
                    layX = worldDirection.x
                    layY = worldDirection.y
                    layZ = worldDirection.z
                    bboxWidth = bbox.width
                    bboxHeight = bbox.height
                    // 行ごとの露光の時刻差を補正するのに、画像のどの行に写ったかが要る。
                    imageX = (bbox.x + bbox.width / 2f) / result.imageWidth
                    imageY = (bbox.y + bbox.height / 2f) / result.imageHeight
                }
            })
        viewModelScope.launch {
            pushFrame(data)
        }
        viewModelScope.launch {
            val yuv = Mat()
            Imgproc.cvtColor(result.mat, yuv, Imgproc.COLOR_BGR2YUV)
            val bytes = ByteArray(yuv.total().toInt())
            yuv.get(0, 0, bytes)
            //videoEncoder.writeFrame(bytes.size, 0, ByteBuffer.allocate(bytes.size).put(bytes))
        }
    }

    fun flashVideo() {
        //videoEncoder.stop()
    }

    companion object {
        const val TAG = "CameraViewModel"
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
