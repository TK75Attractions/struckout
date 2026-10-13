package com.taichi765.struckoutCameraApp.camera

import android.content.Context
import android.content.Context.CAMERA_SERVICE
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraManager
import com.taichi765.struckoutCameraApp.camera.highspeed.HighSpeedCamera
import com.taichi765.struckoutCameraApp.camera.types.WorldDirection
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import dagger.hilt.android.qualifiers.ApplicationContext
import org.opencv.calib3d.Calib3d
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Rect
import timber.log.Timber
import javax.inject.Inject
import kotlin.math.acos
import kotlin.math.sin

class CameraRepository @Inject constructor(
    @ApplicationContext context: Context,
    private val clockInfo: CameraClockInfoHolder,
    private val configRepository: ConfigStoreRepository,
) {
    // 「動かなくなった前景を背景とみなすまでの時間」を 30fps のときと同じ 0.5 秒に保つため、
    // フレーム数は fps に比例させる (30fps で 15 フレーム)。
    val tracker = ObjectTracker(
        0.5, 15.0, 80.0,
        foregroundPersistenceFrames = HighSpeedCamera.TARGET_FPS / 2,
        minColorShift = MIN_BALL_COLOR_SHIFT
    )

    /**
     * @param imageWidth,imageHeight [rect] を検出した解析画像 (回転前) の大きさ
     */
    fun calc(rect: Rect, imageWidth: Int, imageHeight: Int): WorldDirection {
        return calculatorFor(imageWidth, imageHeight).calc(rect)
    }

    private val cameraManager =
        context.getSystemService(CAMERA_SERVICE) as CameraManager

    // CameraScreen は DEFAULT_BACK_CAMERA で撮るので、同じく最初の背面カメラを見る。
    private val characteristics = run {
        val backCameras =
            cameraManager.cameraIdList.map { id -> cameraManager.getCameraCharacteristics(id) }
                .filter { ch -> ch.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_BACK }
        if (backCameras.count() > 1) {
            Timber.tag(TAG).d("There were multiple back camera. selecting first one.")
        }
        backCameras.firstOrNull()
            ?: throw IllegalStateException("This device does not have a back camera.")
    }

    /**
     * センサーの向き。解析画像は、この角度だけ時計回りに回すと端末を縦に持ったときの向きになる。
     */
    val sensorOrientation: Int =
        characteristics.get(CameraCharacteristics.SENSOR_ORIENTATION) ?: 90

    init {
        // ball_tracker がフレームの時刻を PC の時計に直すときに、センサー時刻の時計の種類が要る。
        val source = characteristics.get(CameraCharacteristics.SENSOR_INFO_TIMESTAMP_SOURCE)
        clockInfo.update {
            it.copy(timestampSourceRealtime = source == CameraCharacteristics.SENSOR_INFO_TIMESTAMP_SOURCE_REALTIME)
        }
    }

    // LENS_INTRINSIC_CALIBRATION は pre-correction active array の座標系で表される。
    private val activeArraySize = run {
        characteristics.get(CameraCharacteristics.SENSOR_INFO_PRE_CORRECTION_ACTIVE_ARRAY_SIZE)
            ?: characteristics.get(CameraCharacteristics.SENSOR_INFO_ACTIVE_ARRAY_SIZE)
            ?: throw IllegalStateException("This device does not report the active array size.")
    }

    /**
     * Intrinsics in the coordinate system of [activeArraySize].
     */
    private val sensorIntrinsics: CameraIntrinsics = run {
        val calibration = characteristics.get(CameraCharacteristics.LENS_INTRINSIC_CALIBRATION)
        if (calibration != null && calibration[0] > 0f && calibration[1] > 0f) {
            return@run CameraIntrinsics(
                fx = calibration[0].toDouble(),
                fy = calibration[1].toDouble(),
                cx = calibration[2].toDouble(),
                cy = calibration[3].toDouble()
            )
        }

        Timber.tag(TAG)
            .w("LENS_INTRINSIC_CALIBRATION is not available. Estimating intrinsics from the focal length and the sensor size.")
        val focalLength =
            characteristics.get(CameraCharacteristics.LENS_INFO_AVAILABLE_FOCAL_LENGTHS)
                ?.firstOrNull()
        val physicalSize = characteristics.get(CameraCharacteristics.SENSOR_INFO_PHYSICAL_SIZE)
        val pixelArraySize = characteristics.get(CameraCharacteristics.SENSOR_INFO_PIXEL_ARRAY_SIZE)
        if (focalLength == null || physicalSize == null || pixelArraySize == null) {
            throw IllegalStateException(
                "This device supports neither LENS_INTRINSIC_CALIBRATION nor the sensor info to estimate it." +
                        "You may need to manually measure intrinsics."
            )
        }
        CameraIntrinsics.fromSensorInfo(
            focalLengthMm = focalLength,
            physicalWidthMm = physicalSize.width,
            physicalHeightMm = physicalSize.height,
            pixelArrayWidth = pixelArraySize.width,
            pixelArrayHeight = pixelArraySize.height,
            activeArrayWidth = activeArraySize.width(),
            activeArrayHeight = activeArraySize.height()
        )
    }

    private val cameraRotation: Mat = run {
        val rotation = characteristics.get(CameraCharacteristics.LENS_POSE_ROTATION)
            ?.map { it.toDouble() }
        if (rotation == null) {
            val sensorOrientation =
                characteristics.get(CameraCharacteristics.SENSOR_ORIENTATION) ?: 90
            Timber.tag(TAG)
                .w("LENS_POSE_ROTATION is not available. Assuming the default pose of a back camera (sensor orientation: $sensorOrientation).")
            val rotationMatrix = Mat(3, 3, CvType.CV_64F).apply {
                put(0, 0, *defaultBackCameraRotation(sensorOrientation))
            }
            return@run Mat().also { Calib3d.Rodrigues(rotationMatrix, it) }
        }

        val x = rotation[0]
        val y = rotation[1]
        val z = rotation[2]
        val w = rotation[3]

        // Convert the quaternion to rotation vector. See https://developer.android.com/reference/android/hardware/camera2/CameraCharacteristics#LENS_POSE_ROTATION
        val theta = 2 * acos(w)
        val ax = x / sin(theta / 2)
        val ay = y / sin(theta / 2)
        val az = z / sin(theta / 2)

        Mat(3, 1, CvType.CV_64F).apply {
            put(0, 0, theta * ax)
            put(1, 0, theta * ay)
            put(2, 0, theta * az)
        }
    }

    private var calculator: SizedCalculator? = null

    /**
     * 検出座標は解析画像のピクセルなので、カメラ行列もその解像度に合わせて作る。
     * 解像度は端末が決めるため、最初のフレームが来るまで分からない。
     */
    @Synchronized
    private fun calculatorFor(imageWidth: Int, imageHeight: Int): WorldDirectionCalculator {
        return sizedCalculatorFor(imageWidth, imageHeight).calculator
    }

    /**
     * 解析画像での内部パラメータ。較正用の画像に添える。
     */
    @Synchronized
    fun intrinsicsFor(imageWidth: Int, imageHeight: Int): CameraIntrinsics {
        return sizedCalculatorFor(imageWidth, imageHeight).intrinsics
    }

    private fun sizedCalculatorFor(imageWidth: Int, imageHeight: Int): SizedCalculator {
        // 設定の変更がすぐ反映されるよう、焦点距離の指定は毎回読む。
        val focalLengthRatio = configRepository.focalLengthRatio.value
        calculator?.let {
            if (it.imageWidth == imageWidth && it.imageHeight == imageHeight && it.focalLengthRatio == focalLengthRatio) {
                return it
            }
        }

        val intrinsics = if (focalLengthRatio > 0.0) {
            // 設定で指定された焦点距離を使う。光学中心は画像の中央とする (中央からのずれは、
            // カメラの向きを画像から求めるときにほぼ吸収される)。
            CameraIntrinsics(
                fx = focalLengthRatio * imageWidth,
                fy = focalLengthRatio * imageWidth,
                cx = imageWidth / 2.0,
                cy = imageHeight / 2.0
            )
        } else {
            sensorIntrinsics.scaledToImage(
                activeArrayWidth = activeArraySize.width(),
                activeArrayHeight = activeArraySize.height(),
                imageWidth = imageWidth,
                imageHeight = imageHeight
            )
        }
        Timber.tag(TAG).i("camera intrinsics for ${imageWidth}x$imageHeight: $intrinsics")
        clockInfo.update {
            it.copy(imageWidth = imageWidth, imageHeight = imageHeight, focalLengthPx = intrinsics.fx)
        }
        val cameraMatrix = Mat.eye(3, 3, CvType.CV_64F).apply {
            put(0, 0, intrinsics.fx)
            put(1, 1, intrinsics.fy)
            put(0, 2, intrinsics.cx)
            put(1, 2, intrinsics.cy)
        }
        return SizedCalculator(
            imageWidth,
            imageHeight,
            focalLengthRatio,
            intrinsics,
            WorldDirectionCalculator(cameraMatrix, cameraRotation)
        ).also { calculator = it }
    }

    private data class SizedCalculator(
        val imageWidth: Int,
        val imageHeight: Int,
        val focalLengthRatio: Double,
        val intrinsics: CameraIntrinsics,
        val calculator: WorldDirectionCalculator
    )

    companion object {
        const val TAG = "CameraController"

        // 色つきのボールを残し、影や明るさだけの変化を捨てるための下限 (8 ビットの階調)。
        // 録画した動画 (60fps と 30fps、640×360 に縮めたもの) では、ボールが 14.0 以上
        // (画像の端で切れた 1 コマだけ 7.4)、色のついた箱に落ちたボールの影が 5.5 から 7.4 だった。
        // 10 ならボールの最小値に対して 1.4 倍の余裕がある。12 に上げるとボール以外の検出が
        // さらに 2 割ほど減るが、余裕は 1.2 倍になる。アプリが実際に受け取る解析画像では、
        // まだ確かめていない。背景と色みが同じボール (背景の色を明るく・暗くしただけの色や、
        // 色のない背景の前の白・灰色) は検出できなくなる。
        const val MIN_BALL_COLOR_SHIFT = 10.0
    }
}
