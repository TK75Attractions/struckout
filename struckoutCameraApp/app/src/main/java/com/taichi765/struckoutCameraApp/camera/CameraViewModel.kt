package com.taichi765.struckoutCameraApp.camera

import android.os.SystemClock
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.core.graphics.createBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.camera.types.FrameID
import com.taichi765.struckoutCameraApp.camera.types.increment
import com.taichi765.struckoutCameraApp.camera.types.toULong
import com.taichi765.struckoutCameraApp.config.PushFrameUseCase
import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.proto.detectedObject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.opencv.android.Utils
import org.opencv.core.Mat
import org.opencv.imgproc.Imgproc
import javax.inject.Inject

@HiltViewModel
class CameraViewModel @Inject constructor(
    private val pushFrame: PushFrameUseCase,
    private val cameraRepository: CameraRepository,
    //private val videoEncoderFactory: VideoEncoder.Factory,
) : ViewModel() {
    private val _contoursImage = MutableStateFlow<ImageBitmap?>(null)
    val contoursImage = _contoursImage.asStateFlow()

    private var frameId = FrameID(0u)

    //private lateinit var videoEncoder: VideoEncoder

    val analyzer = MyAnalyzer(cameraRepository.tracker) { result ->
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
            frameId = curFrameID,
            detections = result.bboxes.map { bbox ->
                val worldDirection = cameraRepository.calc(bbox)
                detectedObject {
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
