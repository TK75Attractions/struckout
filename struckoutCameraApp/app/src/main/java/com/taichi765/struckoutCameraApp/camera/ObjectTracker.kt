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

/**
 * 背景との差分で動く物体を見つける。
 *
 * 差分は色ごとに取り、最も大きく変わったチャンネルで判定する。白黒に直してから比べると、
 * 背景と明るさが近い色つきのボール (暗い板の前の青いボールなど) を見落とすため。
 *
 * @param minSaturation 検出した領域の平均色に求める彩度の下限 (0 から 1)。影や揺れのような
 * 色のない変化を捨てるのに使う。0 のときは彩度で絞り込まない
 */
class ObjectTracker(
    val accumulateWeight: Double,
    val binaryThreshold: Double,
    val minContourArea: Double,
    val foregroundPersistenceFrames: Int = 15,
    val minSaturation: Double = 0.0
) {
    private lateinit var background: Mat
    private lateinit var foregroundAge: Mat

    init {
        require(foregroundPersistenceFrames > 0)
        require(minSaturation in 0.0..1.0)
    }

    /**
     * @param frame 3 チャンネルのカラー画像。チャンネルの並びは問わない
     */
    fun nextFrame(frame: Mat): List<Rect> {
        if (!::background.isInitialized) {
            background = Mat()
            frame.convertTo(background, CvType.CV_32F)
            foregroundAge = Mat.zeros(frame.size(), CvType.CV_16UC1)
            return emptyList()
        }

        val background8 = Mat()
        Core.convertScaleAbs(background, background8)

        val colorDelta = Mat()
        Core.absdiff(frame, background8, colorDelta)
        val frameDelta = maxAcrossChannels(colorDelta)

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
        foregroundAge.setTo(Scalar(0.0), backgroundMask)
        Core.add(
            foregroundAge,
            Scalar(1.0),
            foregroundAge,
            thresholdClean,
            CvType.CV_16UC1
        )

        val persistentForegroundMask = Mat()
        Core.compare(
            foregroundAge,
            Scalar(foregroundPersistenceFrames.toDouble()),
            persistentForegroundMask,
            Core.CMP_GE
        )
        val backgroundUpdateMask = Mat()
        Core.bitwise_or(backgroundMask, persistentForegroundMask, backgroundUpdateMask)
        Imgproc.accumulateWeighted(frame, background, accumulateWeight, backgroundUpdateMask)

        val contours = mutableListOf<MatOfPoint>()
        Imgproc.findContours(
            thresholdClean,
            contours,
            Mat(),
            Imgproc.RETR_EXTERNAL,
            Imgproc.CHAIN_APPROX_SIMPLE
        )

        return contours.filter { Imgproc.contourArea(it) >= minContourArea }
            .map { it to Imgproc.boundingRect(it) }
            .filter { (contour, bbox) ->
                minSaturation <= 0.0 || saturation(frame, contour, bbox) >= minSaturation
            }
            .map { (_, bbox) -> bbox }
    }

    private fun maxAcrossChannels(src: Mat): Mat {
        val channels = mutableListOf<Mat>()
        Core.split(src, channels)
        val dst = channels.first()
        channels.drop(1).forEach { Core.max(dst, it, dst) }
        return dst
    }

    /**
     * 輪郭の内側の平均色の彩度 (HSV の S と同じ定義) を 0 から 1 で返す。
     */
    private fun saturation(frame: Mat, contour: MatOfPoint, bbox: Rect): Double {
        val mask = Mat.zeros(bbox.size(), CvType.CV_8UC1)
        Imgproc.drawContours(
            mask,
            listOf(contour),
            0,
            Scalar(255.0),
            Imgproc.FILLED,
            Imgproc.LINE_8,
            Mat(),
            0,
            Point(-bbox.x.toDouble(), -bbox.y.toDouble())
        )
        val mean = Core.mean(frame.submat(bbox), mask).`val`.take(frame.channels())
        val max = mean.max()
        return if (max <= 0.0) 0.0 else (max - mean.min()) / max
    }

    companion object {
        const val TAG = "ObjectTracker"
    }
}
