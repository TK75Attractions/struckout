package com.taichi765.struckoutCameraApp.camera

import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

class CameraIntrinsicsUnitTest {
    // KYV45 の背面カメラが返す値 (adb shell dumpsys media.camera)
    private val kyv45 = CameraIntrinsics.fromSensorInfo(
        focalLengthMm = 3.52f,
        physicalWidthMm = 4.704f,
        physicalHeightMm = 3.536f,
        pixelArrayWidth = 4704,
        pixelArrayHeight = 3536,
        activeArrayWidth = 4672,
        activeArrayHeight = 3504
    )

    @Test
    fun `intrinsics are estimated from the focal length and the sensor size`() {
        assertEquals(3520.0, kyv45.fx, 0.01)
        assertEquals(3520.0, kyv45.fy, 0.01)
        assertEquals(2336.0, kyv45.cx, 1e-9)
        assertEquals(1752.0, kyv45.cy, 1e-9)
    }

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

    @Test
    fun `default pose of a typical back camera`() {
        // センサーの長辺は端末の縦方向に沿い、画像の右は端末の下、画像の下は端末の左を向く
        assertArrayEquals(
            doubleArrayOf(
                0.0, -1.0, 0.0,
                -1.0, 0.0, 0.0,
                0.0, 0.0, -1.0
            ),
            defaultBackCameraRotation(90),
            1e-12
        )
    }

    @Test
    fun `default pose is a proper rotation looking out of the back for every orientation`() {
        for (orientation in listOf(0, 90, 180, 270, -90, 450)) {
            val r = defaultBackCameraRotation(orientation)

            val determinant = r[0] * (r[4] * r[8] - r[5] * r[7]) -
                    r[1] * (r[3] * r[8] - r[5] * r[6]) +
                    r[2] * (r[3] * r[7] - r[4] * r[6])
            assertEquals(1.0, determinant, 1e-12)
            // 光軸 (3 行目) は端末の真後ろ
            assertArrayEquals(doubleArrayOf(0.0, 0.0, -1.0), r.sliceArray(6..8), 1e-12)
        }
    }

    @Test
    fun `default pose rejects an orientation that is not a multiple of 90`() {
        assertThrows(IllegalArgumentException::class.java) {
            defaultBackCameraRotation(45)
        }
    }
}
