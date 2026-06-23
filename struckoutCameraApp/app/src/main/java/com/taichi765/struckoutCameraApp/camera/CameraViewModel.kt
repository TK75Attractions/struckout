package com.taichi765.struckoutCameraApp.camera

import android.os.SystemClock
import androidx.compose.ui.graphics.ImageBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.camera.types.FrameID
import com.taichi765.struckoutCameraApp.camera.types.increment
import com.taichi765.struckoutCameraApp.camera.types.toLong
import com.taichi765.struckoutCameraApp.proto.detectedObject
import com.taichi765.struckoutCameraApp.proto.udpPacket
import com.taichi765.struckoutCameraApp.transport.DetectionRepository
import com.taichi765.struckoutCameraApp.transport.SessionInfoRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class CameraViewModel(
    private val detectionRepository: DetectionRepository,
    private val cameraRepository: CameraRepository,
    sessionInfoRepository: SessionInfoRepository,
) : ViewModel() {
    private val _contoursImage = MutableStateFlow<ImageBitmap?>(null)
    val contoursImage = _contoursImage.asStateFlow()


    private val cameraID = sessionInfoRepository.cameraID.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Eagerly,
        initialValue = 0u // dummy
    )

    private var frameId = FrameID(0u)

    val analyzer = MyAnalyzer(cameraRepository.tracker) { image, imageTimestamp, rects ->
        if (rects.count() == 0) {
            return@MyAnalyzer
        }

        // update properties
        _contoursImage.value = image
        frameId = frameId.increment()

        // create and send packet
        val curFrameID = frameId.toLong()
        val packet = udpPacket {
            cameraId = cameraID.value.toInt()
            timestamp = getTimestamp(imageTimestamp)
            this.frameId = curFrameID
            rects.forEach {
                val worldDirection = cameraRepository.calc(rect = it)
                detectedObjects += detectedObject {
                    layX = worldDirection.x
                    layY = worldDirection.y
                    layZ = worldDirection.z
                    bboxWidth = it.width
                    bboxHeight = it.height
                }
            }
        }
        viewModelScope.launch {
            detectionRepository.pushDetection(packet)
        }
    }

    @Suppress("UNCHECKED_CAST")
    class Factory(
        private val detectionRepository: DetectionRepository,
        private val cameraRepository: CameraRepository,
        private val sessionInfoRepository: SessionInfoRepository
    ) : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return CameraViewModel(
                detectionRepository,
                cameraRepository,
                sessionInfoRepository
            ) as T
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
