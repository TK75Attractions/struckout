package com.taichi765.struckoutCameraApp.camera.highspeed

import com.taichi765.struckoutCameraApp.config.HighSpeedStreamSize

internal const val HIGH_SPEED_TARGET_FPS = 120
internal const val HIGH_SPEED_ANALYSIS_WIDTH = 640

internal data class HighSpeedMode(
    val width: Int,
    val height: Int,
    val minFps: Int,
    val maxFps: Int,
)

/** Select a fixed 120fps mode without requiring Camera2, so device capabilities can be tested. */
internal fun selectHighSpeedMode(
    modes: List<HighSpeedMode>,
    streamSize: HighSpeedStreamSize,
): HighSpeedMode {
    val candidates = modes.filter {
        it.minFps == HIGH_SPEED_TARGET_FPS && it.maxFps == HIGH_SPEED_TARGET_FPS
    }
    require(candidates.isNotEmpty()) { "This device does not support ${HIGH_SPEED_TARGET_FPS}fps capture." }

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
