package com.taichi765.struckoutCameraApp.camera

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

class CameraIntrinsicsUnitTest {
    // 4672x3504 の active array の中央に光学中心がある場合
    private val kyv45 = CameraIntrinsics(fx = 3520.0, fy = 3520.0, cx = 2336.0, cy = 1752.0)

    @Test
    fun `intrinsics are scaled down to an image with the same aspect ratio`() {
        val scaled = kyv45.scaledToImage(4672, 3504, 640, 480)

        assertEquals(482.19, scaled.fx, 0.01)
        assertEquals(482.19, scaled.fy, 0.01)
        assertEquals(320.0, scaled.cx, 1e-9)
        assertEquals(240.0, scaled.cy, 1e-9)
    }

    @Test
    fun `the cropped side is taken into account when the aspect ratio differs`() {
        // 4:3 のセンサーから 16:9 を作ると上下が切り落とされる
        val scaled = kyv45.scaledToImage(4672, 3504, 1280, 720)

        assertEquals(964.38, scaled.fx, 0.01)
        assertEquals(964.38, scaled.fy, 0.01)
        assertEquals(640.0, scaled.cx, 1e-9)
        assertEquals(360.0, scaled.cy, 1e-9)
    }

    @Test
    fun `an off-center principal point stays off-center after scaling`() {
        val calibrated = CameraIntrinsics(fx = 3520.0, fy = 3520.0, cx = 2409.0, cy = 1752.0)

        val scaled = calibrated.scaledToImage(4672, 3504, 640, 480)

        assertEquals(330.0, scaled.cx, 1e-9)
        assertEquals(240.0, scaled.cy, 1e-9)
    }

    @Test
    fun `scaling rejects an empty image`() {
        assertThrows(IllegalArgumentException::class.java) {
            kyv45.scaledToImage(4672, 3504, 0, 480)
        }
    }
}
