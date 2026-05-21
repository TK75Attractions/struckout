package com.taichi765.struckoutCameraApp.camera

import androidx.compose.ui.graphics.ImageBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.ble.FrameData
import com.taichi765.struckoutCameraApp.ble.FrameID
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.opencv.core.Core
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Rect

class CameraViewModel(
    private val bleRepository: BleRepository,
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
        val frameData =
            FrameData(frameId, worldDirection.x, worldDirection.y, worldDirection.z)
        viewModelScope.launch {
            bleRepository.send(frameData)
        }
        frameId = FrameID(frameId.id + 1u)
    }

    @Suppress("UNCHECKED_CAST")
    class Factory(
        private val bleRepository: BleRepository,
        private val cameraRepository: CameraRepository
    ) : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return CameraViewModel(bleRepository, cameraRepository) as T
        }
    }

    companion object {
        const val TAG = "CameraViewModel"
    }
}


class WorldDirectionCalculator(val cameraMatrix: Mat, val cameraRotation: Mat) {
    fun calc(rect: Rect): WorldDirection {
        val pixel = Mat(3, 1, CvType.CV_64F)

        pixel.put(
            0, 0,
            (rect.x + rect.width / 2).toDouble()
        )
        pixel.put(1, 0, (rect.y + rect.height / 2).toDouble())
        pixel.put(2, 0, 1.0)

        val normalized = Mat()
        Core.gemm(cameraMatrix.inv(), pixel, 1.0, Mat(), 0.0, normalized)

        val worldDirection = Mat()
        Core.gemm(cameraRotation, normalized, 1.0, Mat(), 0.0, worldDirection)
        print(worldDirection)
        TODO()
    }
}

data class WorldDirection(val x: Float, val y: Float, val z: Float)

interface BleRepository {
    suspend fun send(frame: FrameData)

}

interface CameraRepository {
    val tracker: ObjectTracker
    fun calc(rect: Rect): WorldDirection
}

