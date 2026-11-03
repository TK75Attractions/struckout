package com.taichi765.struckoutCameraApp.camera.highspeed

import com.taichi765.struckoutCameraApp.config.HighSpeedStreamSize

internal const val HIGH_SPEED_TARGET_FPS = 120

/** 高速撮影に対応していない端末が通常のセッションで出せる fps。CPH2639 は 30fps が上限 */
internal const val NORMAL_TARGET_FPS = 30
internal const val HIGH_SPEED_ANALYSIS_WIDTH = 640

internal data class HighSpeedMode(
    val width: Int,
    val height: Int,
    val minFps: Int,
    val maxFps: Int,
)

/** 端末が constrained high speed に対応していれば 120fps、していなければ通常のセッションの 30fps で撮る。 */
internal fun captureFpsFor(supportsConstrainedHighSpeed: Boolean): Int =
    if (supportsConstrainedHighSpeed) HIGH_SPEED_TARGET_FPS else NORMAL_TARGET_FPS

/**
 * Select a fixed [fps] mode without requiring Camera2, so device capabilities can be tested.
 * 通常のセッションでは、出力の大きさごとに [fps] 固定の範囲を持つ [HighSpeedMode] を渡す。
 */
internal fun selectHighSpeedMode(
    modes: List<HighSpeedMode>,
    streamSize: HighSpeedStreamSize,
    fps: Int = HIGH_SPEED_TARGET_FPS,
): HighSpeedMode {
    val candidates = modes.filter { it.minFps == fps && it.maxFps == fps }
    require(candidates.isNotEmpty()) { "This device does not support ${fps}fps capture." }

    if (streamSize == HighSpeedStreamSize.VGA) {
        candidates.firstOrNull { it.width == 640 && it.height == 480 }?.let { return it }
    }

    // Preserve the existing preference for a full-width 16:9 crop and the smallest usable image.
    return candidates.filter { it.width >= HIGH_SPEED_ANALYSIS_WIDTH }
        .minWithOrNull(
            compareBy<HighSpeedMode> { if (it.width * 9 == it.height * 16) 0 else 1 }
                .thenBy { it.width * it.height }
        ) ?: candidates.maxBy { it.width * it.height }
}
