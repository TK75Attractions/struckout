package com.taichi765.struckoutCameraApp.camera

import android.os.SystemClock
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.core.graphics.createBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.camera.highspeed.HighSpeedCamera
import com.taichi765.struckoutCameraApp.camera.types.FrameID
import com.taichi765.struckoutCameraApp.camera.types.increment
import com.taichi765.struckoutCameraApp.camera.types.toULong
import com.taichi765.struckoutCameraApp.config.PushFrameUseCase
import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.proto.detection
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.opencv.android.Utils
import org.opencv.core.Core
import org.opencv.core.Mat
import org.opencv.core.Rect
import org.opencv.core.Scalar
import org.opencv.imgproc.Imgproc
import java.util.concurrent.TimeUnit
import javax.inject.Inject

@HiltViewModel
class CameraViewModel @Inject constructor(
    private val pushFrame: PushFrameUseCase,
    private val cameraRepository: CameraRepository,
    private val camera: HighSpeedCamera,
) : ViewModel() {
    private val _contoursImage = MutableStateFlow<ImageBitmap?>(null)
    val contoursImage = _contoursImage.asStateFlow()

    val cameraState = camera.state
    val cameraStats = camera.stats

    // 以下は解析スレッドからのみ触る
    private var frameId = FrameID(0u)
    private val gray = Mat()
    private var lastPreviewNanos = 0L

    fun startCamera() {
        camera.start(::onFrame)
    }

    fun stopCamera() {
        camera.stop()
    }

    override fun onCleared() {
        camera.stop()
    }

    /**
     * @param rgba センサーの向きのままの画像。この呼び出しの間だけ有効
     */
    private fun onFrame(rgba: Mat, timestampNanos: Long) {
        Imgproc.cvtColor(rgba, gray, Imgproc.COLOR_RGBA2GRAY)
        val bboxes = cameraRepository.tracker.nextGrayFrame(gray)

        updatePreview(rgba, bboxes)
        if (bboxes.isEmpty()) {
            return
        }

        frameId = frameId.increment()

        // create and send packet
        val curFrameID = frameId.toULong()
        val data = DetectionData(
            timestamp = getTimestamp(TimeUnit.NANOSECONDS.toMillis(timestampNanos)),
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
    private fun updatePreview(rgba: Mat, bboxes: List<Rect>) {
        val now = System.nanoTime()
        val interval = if (bboxes.isEmpty()) IDLE_PREVIEW_INTERVAL_NANOS else PREVIEW_INTERVAL_NANOS
        if (now - lastPreviewNanos < interval) {
            return
        }
        lastPreviewNanos = now

        // 検出はグレースケール側で済んでいるので、ここで描き込んでも結果には影響しない。
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
