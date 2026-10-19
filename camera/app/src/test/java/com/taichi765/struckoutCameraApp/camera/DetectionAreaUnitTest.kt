package com.taichi765.struckoutCameraApp.camera

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class DetectionAreaUnitTest {
    @Test
    fun `accepts everything by default`() {
        val area = DetectionArea()

        assertTrue(area.coversWholeImage)
        assertTrue(area.contains(0.0, 0.0, 640, 480))
        assertTrue(area.contains(640.0, 480.0, 640, 480))
    }

    @Test
    fun `the top of the preview is the left of the analysis image`() {
        // プレビューは解析画像を時計回りに 90 度回したものなので、解析画像の左端がプレビューの上端になる。
        val area = DetectionArea(topPercent = 25.0)

        assertFalse(area.contains(100.0, 240.0, 640, 480))
        assertTrue(area.contains(200.0, 240.0, 640, 480))
        assertEquals(160.0, area.inAnalysisImage(640, 480).left, 1e-9)
    }

    @Test
    fun `the bottom of the preview is the right of the analysis image`() {
        val area = DetectionArea(bottomPercent = 25.0)

        assertTrue(area.contains(400.0, 240.0, 640, 480))
        assertFalse(area.contains(500.0, 240.0, 640, 480))
        assertEquals(480.0, area.inAnalysisImage(640, 480).right, 1e-9)
    }

    @Test
    fun `the left of the preview is the bottom of the analysis image`() {
        val area = DetectionArea(leftPercent = 25.0)

        assertTrue(area.contains(320.0, 300.0, 640, 480))
        assertFalse(area.contains(320.0, 400.0, 640, 480))
        assertEquals(360.0, area.inAnalysisImage(640, 480).bottom, 1e-9)
    }

    @Test
    fun `the right of the preview is the top of the analysis image`() {
        val area = DetectionArea(rightPercent = 25.0)

        assertFalse(area.contains(320.0, 100.0, 640, 480))
        assertTrue(area.contains(320.0, 200.0, 640, 480))
        assertEquals(120.0, area.inAnalysisImage(640, 480).top, 1e-9)
    }

    @Test
    fun `rejects margins that leave nothing of the image`() {
        assertFalse(DetectionArea.isValid(60.0, 0.0, 40.0, 0.0))
        assertFalse(DetectionArea.isValid(0.0, -1.0, 0.0, 0.0))
        assertTrue(DetectionArea.isValid(60.0, 0.0, 39.0, 0.0))
        assertThrows(IllegalArgumentException::class.java) {
            DetectionArea(topPercent = 50.0, bottomPercent = 50.0)
        }
    }
}
