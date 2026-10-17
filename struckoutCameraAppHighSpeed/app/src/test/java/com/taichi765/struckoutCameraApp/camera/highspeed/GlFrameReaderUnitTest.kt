package com.taichi765.struckoutCameraApp.camera.highspeed

import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Test

class GlFrameReaderUnitTest {
    // 行列は OpenGL と同じ列優先

    @Test
    fun `whole texture is valid when the transform only flips vertically`() {
        val flip = floatArrayOf(
            1f, 0f, 0f, 0f,
            0f, -1f, 0f, 0f,
            0f, 0f, 1f, 0f,
            0f, 1f, 0f, 1f
        )

        assertArrayEquals(floatArrayOf(0f, 0f, 1f, 1f), GlFrameReader.validTextureRect(flip), 1e-6f)
    }

    @Test
    fun `rotation in the transform does not change the valid rect`() {
        // 端末の向きに合わせた 90 度回転 (u = t, v = s)
        val rotate = floatArrayOf(
            0f, 1f, 0f, 0f,
            1f, 0f, 0f, 0f,
            0f, 0f, 1f, 0f,
            0f, 0f, 0f, 1f
        )

        assertArrayEquals(floatArrayOf(0f, 0f, 1f, 1f), GlFrameReader.validTextureRect(rotate), 1e-6f)
    }

    @Test
    fun `padding at the buffer edge is excluded`() {
        // 高さ 1088 のバッファのうち上から 1080 行だけが画像、という切り出しと上下反転
        val valid = 1080f / 1088f
        val cropAndFlip = floatArrayOf(
            1f, 0f, 0f, 0f,
            0f, -valid, 0f, 0f,
            0f, 0f, 1f, 0f,
            0f, valid, 0f, 1f
        )

        assertArrayEquals(
            floatArrayOf(0f, 0f, 1f, valid),
            GlFrameReader.validTextureRect(cropAndFlip),
            1e-6f
        )
    }
}
