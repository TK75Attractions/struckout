package com.taichi765.struckoutCameraApp.camera

import android.hardware.camera2.CaptureResult
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.abs
import kotlin.math.max

/**
 * ball_tracker に知らせる、カメラの設定。ball_tracker はこれを使って、フレームの時刻を PC の時計に直す。
 *
 * @param timestampSourceRealtime センサー時刻が `SystemClock.elapsedRealtimeNanos()` と同じ時計で
 * 付いているか。まだ分からないうちは `null`
 * @param rollingShutterSkewNanos 画像の最初の行と最後の行の露光開始の時刻の差。端末が返さないときは 0
 * @param exposureTimeNanos 最後に見た露光時間。端末が返さないときは 0
 * @param focalLengthPx 視線の計算に使っている焦点距離 (解析画像の px)
 */
data class CameraClockInfo(
    val appVariant: String = "",
    val timestampSourceRealtime: Boolean? = null,
    val rollingShutterSkewNanos: Long = 0,
    val exposureTimeNanos: Long = 0,
    val imageWidth: Int = 0,
    val imageHeight: Int = 0,
    val focalLengthPx: Double = 0.0,
    val frameRate: Double = 0.0,
)

/**
 * [CameraClockInfo] を、カメラを扱う側から通信を扱う側へ渡す。
 */
@Singleton
class CameraClockInfoHolder @Inject constructor() {
    private val _info = MutableStateFlow(CameraClockInfo())
    val info = _info.asStateFlow()

    fun update(transform: (CameraClockInfo) -> CameraClockInfo) {
        _info.update(transform)
    }

    /**
     * 撮影結果から、行ごとの露光の時刻差と露光時間を取り込む。
     * フレームごとに呼ばれるので、値が変わったときだけ流す。
     */
    fun onCaptureResult(result: CaptureResult) {
        val skew = result.get(CaptureResult.SENSOR_ROLLING_SHUTTER_SKEW) ?: 0L
        val exposure = result.get(CaptureResult.SENSOR_EXPOSURE_TIME) ?: 0L
        val current = _info.value
        // 自動露出では露光時間が細かく揺れるので、1 割以上変わったときだけ知らせる。
        val exposureChanged =
            abs(exposure - current.exposureTimeNanos) * 10 > max(exposure, current.exposureTimeNanos)
        if (skew != current.rollingShutterSkewNanos || exposureChanged) {
            _info.update {
                it.copy(rollingShutterSkewNanos = skew, exposureTimeNanos = exposure)
            }
        }
    }
}
