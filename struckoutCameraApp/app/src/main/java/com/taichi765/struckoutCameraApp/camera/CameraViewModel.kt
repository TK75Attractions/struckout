package com.taichi765.struckoutCameraApp.camera

import android.os.SystemClock
import androidx.compose.ui.graphics.ImageBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.camera.types.FrameID
import com.taichi765.struckoutCameraApp.camera.types.increment
import com.taichi765.struckoutCameraApp.transport.ConnectionState
import com.taichi765.struckoutCameraApp.transport.TcpTransportRepository
import com.taichi765.struckoutCameraApp.transport.UdpTransportRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import struckout.v1.detectedObject
import struckout.v1.udpPacket

class CameraViewModel(
    private val udpRepository: UdpTransportRepository,
    private val tcpRepository: TcpTransportRepository,
    private val cameraRepository: CameraRepository
) : ViewModel() {
    private val _contoursImage = MutableStateFlow<ImageBitmap?>(null)
    val contoursImage = _contoursImage.asStateFlow()

    private var frameId = FrameID(0u)

    val analyzer = MyAnalyzer(cameraRepository.tracker) { image, imageTimestamp, rects ->
        val connState = tcpRepository.state.value
        check(connState is ConnectionState.Connected) {
            "TCP connection must be established before camera starts"
        }
        if (rects.count() == 0) {
            return@MyAnalyzer
        }
        _contoursImage.value = image
        frameId = frameId.increment()
        val packet = udpPacket {
            cameraId = connState.cameraID.toInt()
            timestamp = getTimestamp(imageTimestamp)
            frameId = frameId
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
            udpRepository.sendPacket(packet)
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

    @Suppress("UNCHECKED_CAST")
    class Factory(
        private val udpRepository: UdpTransportRepository,
        private val tcpRepository: TcpTransportRepository,
        private val cameraRepository: CameraRepository
    ) : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return CameraViewModel(udpRepository, tcpRepository, cameraRepository) as T
        }
    }

    companion object {
        const val TAG = "CameraViewModel"
    }
}
