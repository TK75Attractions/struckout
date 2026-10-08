package com.taichi765.struckoutCameraApp.camera

import org.opencv.core.Core
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.MatOfPoint
import org.opencv.core.Point
import org.opencv.core.Rect
import org.opencv.core.Scalar
import org.opencv.core.Size
import org.opencv.imgproc.Imgproc
import org.opencv.imgproc.Imgproc.COLOR_RGB2GRAY

/**
 * 背景との差分で動く物体を見つける。
 *
 * 毎秒 120 フレームで呼ばれるので、作業用の [Mat] はフレームごとに作らず使い回す。
 * そのため同時に複数のスレッドから呼んではいけない。
 */
class ObjectTracker(
    val accumulateWeight: Double,
    val binaryThreshold: Double,
    val minContourArea: Double,
    val foregroundPersistenceFrames: Int = 15
) {
    private var initialized = false
    private val background = Mat()
    private val foregroundAge = Mat()

    private val gray = Mat()
    private val background8 = Mat()
    private val frameDelta = Mat()
    private val threshold = Mat()
    private val thresholdOpen = Mat()
    private val thresholdClean = Mat()
    private val backgroundMask = Mat()
    private val persistentForegroundMask = Mat()
    private val backgroundUpdateMask = Mat()
    private val hierarchy = Mat()
    private val kernel =
        Imgproc.getStructuringElement(Imgproc.MORPH_ELLIPSE, Size(5.0, 5.0), Point(-1.0, -1.0))

    init {
        require(foregroundPersistenceFrames > 0)
    }

    /**
     * @param frame RGB 画像
     */
    fun nextFrame(frame: Mat): List<Rect> {
        Imgproc.cvtColor(frame, gray, COLOR_RGB2GRAY)
        return nextGrayFrame(gray)
    }

    /**
     * @param gray グレースケール画像 (CV_8UC1)。書き換えない
     */
    fun nextGrayFrame(gray: Mat): List<Rect> {
        if (!initialized || background.rows() != gray.rows() || background.cols() != gray.cols()) {
            gray.convertTo(background, CvType.CV_32F)
            Mat.zeros(gray.size(), CvType.CV_16UC1).copyTo(foregroundAge)
            initialized = true
            return emptyList()
        }

        Core.convertScaleAbs(background, background8)
        Core.absdiff(gray, background8, frameDelta)
        Imgproc.threshold(frameDelta, threshold, binaryThreshold, 255.0, Imgproc.THRESH_BINARY)

        Imgproc.morphologyEx(
            threshold,
            thresholdOpen,
            Imgproc.MORPH_OPEN,
            kernel,
            Point(-1.0, -1.0),
            1,
            Core.BORDER_CONSTANT
        )
        Imgproc.morphologyEx(
            thresholdOpen,
            thresholdClean,
            Imgproc.MORPH_CLOSE,
            kernel,
            Point(-1.0, -1.0),
            2,
            Core.BORDER_CONSTANT
        )

        Core.bitwise_not(thresholdClean, backgroundMask)
        foregroundAge.setTo(Scalar(0.0), backgroundMask)
        Core.add(
            foregroundAge,
            Scalar(1.0),
            foregroundAge,
            thresholdClean,
            CvType.CV_16UC1
        )

        Core.compare(
            foregroundAge,
            Scalar(foregroundPersistenceFrames.toDouble()),
            persistentForegroundMask,
            Core.CMP_GE
        )
        Core.bitwise_or(backgroundMask, persistentForegroundMask, backgroundUpdateMask)
        Imgproc.accumulateWeighted(gray, background, accumulateWeight, backgroundUpdateMask)

        val contours = mutableListOf<MatOfPoint>()
        Imgproc.findContours(
            thresholdClean,
            contours,
            hierarchy,
            Imgproc.RETR_EXTERNAL,
            Imgproc.CHAIN_APPROX_SIMPLE
        )

        val rects = contours.filter { Imgproc.contourArea(it) >= minContourArea }
            .map { Imgproc.boundingRect(it) }
        contours.forEach { it.release() }
        return rects
    }

    companion object {
        const val TAG = "ObjectTracker"
    }
}
