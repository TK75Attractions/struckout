package com.taichi765.struckoutCameraApp.camera

import androidx.compose.ui.graphics.ImageBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.ble.CameraLocation
import com.taichi765.struckoutCameraApp.ble.FrameData
import com.taichi765.struckoutCameraApp.ble.FrameID
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.opencv.calib3d.Calib3d
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

/**
 * @constructor cameraRotationVector is the vector of `(theta*ax, theta*ay, theta*az)`.
 *
 * See [Android Document](https://developer.android.com/reference/android/hardware/camera2/CameraCharacteristics#LENS_POSE_ROTATION) for more information.
 *
 */
class WorldDirectionCalculator(val cameraMatrix: Mat, cameraRotationVector: Mat) {
    init {
        require(cameraMatrix.height() == 3 && cameraMatrix.width() == 3) { "size of cameraMatrix was incorrect: expected 3x3" }
        require(cameraRotationVector.height() == 3 && cameraRotationVector.width() == 1) { "size of cameraRotationVector was incorrect: expected 3x1" }
    }

    // convert rotation vector to rotation matrix.
    private val cameraRotationMatrix = run {
        val dst = Mat()
        Calib3d.Rodrigues(cameraRotationVector, dst)
        dst
    }


    fun calc(rect: Rect): WorldDirection {
        // カメラ座標系でのピクセル値 (u,v,1)
        val cameraCoordinate = Mat(3, 1, CvType.CV_64F).apply {
            put(0, 0, (rect.x + rect.width / 2).toDouble())
            put(1, 0, (rect.y + rect.height / 2).toDouble())
            put(2, 0, 1.0)
        }

        // 正規化カメラ座標
        val normalizedCameraCoordinate = run {
            val dst = Mat()
            Core.gemm(cameraMatrix.inv(), cameraCoordinate, 1.0, Mat(), 0.0, dst)
            dst
        }

        val worldDirection = run {
            val dst = Mat()
            Core.gemm(cameraRotationMatrix, normalizedCameraCoordinate, 1.0, Mat(), 0.0, dst)
            dst
        }

        return WorldDirection(
            x = worldDirection.get(0, 0).first().toFloat(),
            y = worldDirection.get(1, 0).first().toFloat(),
            z = worldDirection.get(2, 0).first().toFloat()
        )
    }
}

data class WorldDirection(val x: Float, val y: Float, val z: Float)

interface BleRepository {
    suspend fun send(frame: FrameData)
    suspend fun sendCameraLocation(loc: CameraLocation)
    suspend fun connect()
}

interface CameraRepository {
    val tracker: ObjectTracker
    fun calc(rect: Rect): WorldDirection
}

