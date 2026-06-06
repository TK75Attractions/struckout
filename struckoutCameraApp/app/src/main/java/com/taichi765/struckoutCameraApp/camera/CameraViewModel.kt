package com.taichi765.struckoutCameraApp.camera

import androidx.compose.ui.graphics.ImageBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.camera.types.FrameID
import com.taichi765.struckoutCameraApp.transport.UdpTransportRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import struckout.v1.detectedObject
import struckout.v1.udpPacket

class CameraViewModel(
    private val udpTransportRepository: UdpTransportRepository,
    val cameraRepository: CameraRepository
) : ViewModel() {
    private val _contoursImage = MutableStateFlow<ImageBitmap?>(null)
    val contoursImage = _contoursImage.asStateFlow()

    private var frameId = FrameID(0u)

    val analyzer = MyAnalyzer(cameraRepository.tracker) { image, rects ->
        if (rects.count() == 0) {
            return@MyAnalyzer
        }
        _contoursImage.value = image
        val worldDirection = cameraRepository.calc(rects[0])//TODO: オブジェクトが複数ある場合の処理
        val packet = udpPacket {
            cameraId = TODO()
            timestamp = TODO()
            frameId = TODO()
            detectedObjects += detectedObject {
                TODO()
            }
        }
        viewModelScope.launch {
            udpTransportRepository.sendPacket(packet)
        }
        frameId = FrameID(frameId.id + 1u)
    }

    @Suppress("UNCHECKED_CAST")
    class Factory(
        private val udpRepository: UdpTransportRepository,
        private val cameraRepository: CameraRepository
    ) : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return CameraViewModel(udpRepository, cameraRepository) as T
        }
    }

    companion object {
        const val TAG = "CameraViewModel"
    }
}
