package com.taichi765.struckoutCameraApp.camera

import android.content.Context
import android.graphics.Bitmap
import android.os.Build
import androidx.core.graphics.createBitmap
import dagger.hilt.android.qualifiers.ApplicationContext
import org.json.JSONObject
import org.opencv.android.Utils
import org.opencv.core.Mat
import timber.log.Timber
import java.io.File
import java.io.IOException
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import javax.inject.Inject

/**
 * カメラの位置と向きを PC で求める (`scripts/recording_analysis/calibrate_camera.py`) ための画像を保存する。
 *
 * 標準カメラの写真は、解析に使う画像と切り出しや画角が違うことがあるので使えない。
 * 解析画像そのものを、検出の枠を描き込む前に、センサーの向きのまま保存する。
 */
class CalibrationFrameSaver @Inject constructor(
    @ApplicationContext private val context: Context,
    private val clockInfo: CameraClockInfoHolder,
) {
    /**
     * 画像を PNG で、撮影の設定を同じ名前の JSON で保存する。
     * 保存先はアプリの外部ファイル領域の `calibration` で、`scripts/pull-calibration-frames.sh` で PC に取り込める。
     *
     * @param rgb センサーの向きのままの解析画像 (RGB)
     * @param intrinsics [rgb] の大きさでの内部パラメータ
     * @return 画面に出す結果。保存できたときは、保存した場所
     */
    fun save(rgb: Mat, intrinsics: CameraIntrinsics, sensorOrientation: Int): String {
        val dir = context.getExternalFilesDir("calibration") ?: return "保存先がありません"
        val name = "frame_" + SimpleDateFormat("yyyyMMdd_HHmmss_SSS", Locale.US).format(Date())
        return try {
            val bitmap = createBitmap(rgb.cols(), rgb.rows())
            Utils.matToBitmap(rgb, bitmap)
            val image = File(dir, "$name.png")
            image.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }

            val info = clockInfo.info.value
            val json = JSONObject().apply {
                put("image_width", rgb.cols())
                put("image_height", rgb.rows())
                put("fx", intrinsics.fx)
                put("fy", intrinsics.fy)
                put("cx", intrinsics.cx)
                put("cy", intrinsics.cy)
                put("sensor_orientation", sensorOrientation)
                put(
                    "timestamp_source",
                    when (info.timestampSourceRealtime) {
                        true -> "realtime"
                        false -> "unknown"
                        null -> JSONObject.NULL
                    }
                )
                put("rolling_shutter_skew_ns", info.rollingShutterSkewNanos)
                put("exposure_time_ns", info.exposureTimeNanos)
                put("frame_rate", info.frameRate)
                put("app_variant", info.appVariant)
                put("device_model", Build.MODEL)
            }
            File(dir, "$name.json").writeText(json.toString(2))
            image.absolutePath
        } catch (e: IOException) {
            Timber.tag(TAG).w("failed to save a calibration frame: $e")
            "保存に失敗しました: $e"
        }
    }

    companion object {
        const val TAG = "CalibrationFrameSaver"
    }
}
