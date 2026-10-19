package com.taichi765.struckoutCameraApp.camera.highspeed

import android.os.SystemClock
import timber.log.Timber
import kotlin.math.abs

/**
 * 撮影結果 (`onCaptureCompleted`) のセンサー時刻を覚えておき、エンコーダーやデコーダーを通ったフレームの時刻を
 * センサー時刻に戻す。
 *
 * エンコーダーとデコーダーの時刻はマイクロ秒に丸められている。また、センサー時刻が elapsedRealtime の時計の
 * 端末では、動画エンコーダーに渡すときに Android が uptime の時計に直す。ball_tracker にはこれまでどおり
 * センサー時刻を送りたいので、直近の撮影結果のうち、そのままか、2 つの時計の差を足すと 1ms 以内で合うものを
 * 探して、その時刻を返す。見つからないときは、前に合った補正を足す。
 *
 * [record] はカメラのスレッドから、[sensorTimestamp] はほかのスレッド (GL、コーデック) から呼んでよい。
 */
class CaptureTimeline {
    private val timestamps = LongArray(HISTORY)
    private var count = 0L

    // ロックの中で触る。エンコーダーの時刻に足すとセンサー時刻になる量
    private var correction: Long? = null

    fun record(timestamp: Long) {
        synchronized(this) {
            timestamps[(count % HISTORY).toInt()] = timestamp
            count++
        }
    }

    /** @param encoded エンコーダーかデコーダーが付けた時刻 (ナノ秒) */
    fun sensorTimestamp(encoded: Long): Long {
        val bootOffset = SystemClock.elapsedRealtimeNanos() - System.nanoTime()
        synchronized(this) {
            val candidates = listOfNotNull(correction, 0L, bootOffset).distinct()
            val recorded = minOf(count, HISTORY.toLong()).toInt()
            for (candidate in candidates) {
                for (i in 0 until recorded) {
                    val captured = timestamps[i]
                    if (abs(encoded + candidate - captured) <= TOLERANCE_NANOS) {
                        if (correction == null) {
                            Timber.tag(TAG).i("encoded frame timestamps are corrected by %dns", candidate)
                        }
                        correction = candidate
                        return captured
                    }
                }
            }
            return encoded + (correction ?: 0L)
        }
    }

    companion object {
        private const val TAG = "CaptureTimeline"

        /** 覚えておく撮影結果の数。120fps で約 2 秒。エンコードとデコードの遅れより十分長くする */
        private const val HISTORY = 256

        /** 同じフレームとみなす時刻の差。フレームの間隔 (8.3ms) より十分小さくする */
        private const val TOLERANCE_NANOS = 1_000_000L
    }
}
