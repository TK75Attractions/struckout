package com.taichi765.struckoutCameraApp.camera

import android.os.SystemClock
import androidx.compose.ui.graphics.ImageBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.camera.types.FrameID
import com.taichi765.struckoutCameraApp.camera.types.increment
import com.taichi765.struckoutCameraApp.camera.types.toULong
import com.taichi765.struckoutCameraApp.proto.detectedObject
import com.taichi765.struckoutCameraApp.transport.DetectionData
import com.taichi765.struckoutCameraApp.transport.DetectionRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class CameraViewModel @Inject constructor(
    private val detectionRepository: DetectionRepository,
    private val cameraRepository: CameraRepository,
) : ViewModel() {
    private val _contoursImage = MutableStateFlow<ImageBitmap?>(null)
    val contoursImage = _contoursImage.asStateFlow()

    private var frameId = FrameID(0u)

    val analyzer = MyAnalyzer(cameraRepository.tracker) { image, imageTimestamp, rects ->
        if (rects.count() == 0) {
            return@MyAnalyzer
        }

        // update properties
        _contoursImage.value = image
        frameId = frameId.increment()

        // create and send packet
        val curFrameID = frameId.toULong()
        val data = DetectionData(
            timestamp = getTimestamp(imageTimestamp),
            frameId = curFrameID,
            detections = rects.map { rect ->
                val worldDirection = cameraRepository.calc(rect)
                detectedObject {
                    layX = worldDirection.x
                    layY = worldDirection.y
                    layZ = worldDirection.z
                    bboxWidth = rect.width
                    bboxHeight = rect.height
                }
            })
        viewModelScope.launch {
            detectionRepository.pushDetection(data)
        }
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
