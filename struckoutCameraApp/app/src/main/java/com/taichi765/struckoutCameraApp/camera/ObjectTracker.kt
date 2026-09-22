package com.taichi765.struckoutCameraApp.camera

import org.opencv.core.Core
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.MatOfPoint
import org.opencv.core.Point
import org.opencv.core.Rect
import org.opencv.core.Size
import org.opencv.imgproc.Imgproc
import org.opencv.imgproc.Imgproc.COLOR_RGB2GRAY

class ObjectTracker(
    val accumulateWeight: Double,
    val binaryThreshold: Double,
    val minContourArea: Double
) {
    private lateinit var background: Mat

    fun nextFrame(frame: Mat): List<Rect> {
        val gray = Mat()
        Imgproc.cvtColor(frame, gray, COLOR_RGB2GRAY)

        if (!::background.isInitialized) {
            background = Mat()
            gray.convertTo(background, CvType.CV_32F)
            return emptyList()
        }

        val background8 = Mat()
        Core.convertScaleAbs(background, background8)

        val frameDelta = Mat()
        Core.absdiff(gray, background8, frameDelta)

        val threshold = Mat()
        Imgproc.threshold(frameDelta, threshold, binaryThreshold, 255.0, Imgproc.THRESH_BINARY)

        val kernel =
            Imgproc.getStructuringElement(Imgproc.MORPH_ELLIPSE, Size(5.0, 5.0), Point(-1.0, -1.0))
        val thresholdOpen = Mat()
        Imgproc.morphologyEx(
            threshold,
            thresholdOpen,
            Imgproc.MORPH_OPEN,
            kernel,
            Point(-1.0, -1.0),
            1,
            Core.BORDER_CONSTANT
        )
        val thresholdClean = Mat()
        Imgproc.morphologyEx(
            thresholdOpen,
            thresholdClean,
            Imgproc.MORPH_CLOSE,
            kernel,
            Point(-1.0, -1.0),
            2,
            Core.BORDER_CONSTANT
        )

        val backgroundMask = Mat()
        Core.bitwise_not(thresholdClean, backgroundMask)
        Imgproc.accumulateWeighted(gray, background, accumulateWeight, backgroundMask)

        val contours = mutableListOf<MatOfPoint>()
        Imgproc.findContours(
            thresholdClean,
            contours,
            Mat(),
            Imgproc.RETR_EXTERNAL,
            Imgproc.CHAIN_APPROX_SIMPLE
        )

        return contours.filter { Imgproc.contourArea(it) >= minContourArea }
            .map { Imgproc.boundingRect(it) }.toList()
    }

    companion object {
        const val TAG = "ObjectTracker"
    }
}
