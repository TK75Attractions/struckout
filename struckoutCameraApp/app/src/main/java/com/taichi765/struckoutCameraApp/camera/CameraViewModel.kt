package com.taichi765.struckoutCameraApp.camera

import android.os.SystemClock
import androidx.compose.ui.graphics.ImageBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.camera.types.FrameID
import com.taichi765.struckoutCameraApp.camera.types.increment
import com.taichi765.struckoutCameraApp.camera.types.toLong
import com.taichi765.struckoutCameraApp.transport.ConnectionState
import com.taichi765.struckoutCameraApp.transport.TcpTransportRepository
import com.taichi765.struckoutCameraApp.transport.UdpTransportRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.stateIn
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

    val tcpConnState = tcpRepository.state.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Eagerly,
        initialValue = ConnectionState.Disconnected
    )

    val udpIsBound = udpRepository.isBound.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Eagerly,
        initialValue = false
    )

    private var frameId = FrameID(0u)

    val analyzer = MyAnalyzer(cameraRepository.tracker) { image, imageTimestamp, rects ->
        // check connection states
        val currentConnState = tcpConnState.value
        check(currentConnState is ConnectionState.Connected) {
            "TCP connection must be established before camera starts"
        }
        check(udpIsBound.value) {
            "UDP socket must be bound to port before camera starts"
        }
        if (rects.count() == 0) {
            return@MyAnalyzer
        }

        // update properties
        _contoursImage.value = image
        frameId = frameId.increment()

        // create and send packet
        val curFrameID = frameId.toLong()
        val packet = udpPacket {
            cameraId = currentConnState.cameraID.toInt()
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
            udpRepository.sendPacket(packet)
        }
    }

    fun bindUdpSocket() {
        if (udpIsBound.value) {
            return
        }
        viewModelScope.launch {
            udpRepository.bind()
        }
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

/**
 * @return elapsed time from UNIX epoch to image's timestamp
 */
private fun getTimestamp(imageTimestamp: Long): Long {
    // TODO: 平均取って精度上げてもいいかも
    val boot = System.currentTimeMillis() - SystemClock.elapsedRealtime()
    return boot + imageTimestamp
}
