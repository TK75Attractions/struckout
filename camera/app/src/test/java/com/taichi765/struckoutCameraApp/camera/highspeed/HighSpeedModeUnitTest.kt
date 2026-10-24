package com.taichi765.struckoutCameraApp.camera.highspeed

import com.taichi765.struckoutCameraApp.config.HighSpeedStreamSize
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

class HighSpeedModeUnitTest {
    private fun mode(width: Int, height: Int, minFps: Int = 120, maxFps: Int = 120) =
        HighSpeedMode(width, height, minFps, maxFps)

    private val kyv45 = listOf(
        mode(1280, 720), mode(1920, 1080), mode(800, 480), mode(720, 480),
        mode(640, 480), mode(480, 360), mode(480, 320), mode(320, 240),
        mode(1280, 480), mode(1280, 400),
    )

    @Test
    fun `the default preserves the KYV45 720p mode`() {
        assertEquals(mode(1280, 720), selectHighSpeedMode(kyv45, HighSpeedStreamSize.HD720))
    }

    @Test
    fun `VGA selects fixed 120fps 640x480 on KYV45`() {
        assertEquals(mode(640, 480), selectHighSpeedMode(kyv45, HighSpeedStreamSize.VGA))
    }

    @Test
    fun `VGA falls back to the default if 640x480 is absent`() {
        val modes = kyv45.filterNot { it.width == 640 && it.height == 480 }
        assertEquals(mode(1280, 720), selectHighSpeedMode(modes, HighSpeedStreamSize.VGA))
    }

    @Test
    fun `VGA falls back if 640x480 only supports variable or different fps`() {
        val modes = listOf(mode(640, 480, 30, 120), mode(640, 480, 240, 240), mode(1280, 720))
        assertEquals(mode(1280, 720), selectHighSpeedMode(modes, HighSpeedStreamSize.VGA))
    }

    @Test
    fun `the default selects the smallest usable size without a 16 by 9 mode`() {
        assertEquals(
            mode(640, 480),
            selectHighSpeedMode(listOf(mode(800, 480), mode(640, 480)), HighSpeedStreamSize.HD720),
        )
    }

    @Test
    fun `the default preserves the largest size fallback below analysis width`() {
        assertEquals(
            mode(480, 360),
            selectHighSpeedMode(listOf(mode(320, 240), mode(480, 360)), HighSpeedStreamSize.HD720),
        )
    }

    @Test
    fun `a device without fixed 120fps is rejected`() {
        assertThrows(IllegalArgumentException::class.java) {
            selectHighSpeedMode(listOf(mode(640, 480, 30, 120)), HighSpeedStreamSize.VGA)
        }
    }
}
