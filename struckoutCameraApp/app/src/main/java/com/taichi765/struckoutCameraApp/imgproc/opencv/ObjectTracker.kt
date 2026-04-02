package com.taichi765.struckoutCameraApp.imgproc.opencv

import org.opencv.core.Core
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.MatOfPoint
import org.opencv.core.Point
import org.opencv.core.Rect
import org.opencv.core.Size
import org.opencv.imgproc.Imgproc
import org.opencv.imgproc.Imgproc.COLOR_BGR2GRAY

class ObjectTracker(
    val accumulateWeight: Double,
    val binaryThreshold: Double,
    val minContourArea: Double
) {
    private lateinit var prev: Mat

    fun nextFrame(frame: Mat): List<Rect> {
        if (!::prev.isInitialized) {
            val gray32 = Mat()
            Imgproc.cvtColor(frame, gray32, COLOR_BGR2GRAY)
            gray32.convertTo(gray32, CvType.CV_32F)
            prev = gray32
        }

        // CV_32FC1
        if (prev.depth() != 5) {
            prev.convertTo(prev, CvType.CV_32F)
        }

        // CV_8UC1
        val gray = Mat()
        Imgproc.cvtColor(frame, gray, COLOR_BGR2GRAY)

        Imgproc.accumulateWeighted(gray, prev, accumulateWeight)

        // CV_32FC1 -> CV_8UC1
        val prev8 = Mat()
        Core.convertScaleAbs(prev, prev8)

        // src: CV_8UC1, dst: CV_8UC1
        val frameDelta = Mat()
        Core.absdiff(gray, prev8, frameDelta)

        // CV_8UC1
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
            Point(-1.0, -1.0), 1, Core.BORDER_CONSTANT
        )
        val thresholdClean = Mat()
        Imgproc.morphologyEx(
            thresholdOpen,
            thresholdClean,
            Imgproc.MORPH_CLOSE,
            kernel,
            Point(-1.0, -1.0),
            2, Core.BORDER_CONSTANT
        )

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