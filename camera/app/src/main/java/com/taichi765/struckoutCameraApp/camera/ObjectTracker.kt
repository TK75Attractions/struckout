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
import kotlin.math.min
import kotlin.math.sqrt

/**
 * 背景との差分で動く物体を見つける。
 *
 * 差分は色ごとに取り、最も大きく変わったチャンネルで判定する。白黒に直してから比べると、
 * 背景と明るさが近い色つきのボール (暗い板の前の青いボールなど) を見落とすため。
 *
 * 見つけた領域は、平均色が背景の同じ場所の平均色から色みをどれだけ変えたかで絞り込む。
 * 飛んでいるボールは露光中に動いてぶれ、平均色に透けた背景が混ざって色が薄まる。
 * 平均色の鮮やかさだけを見ると、明るい背景の前のボールより、色のついた面に落ちた影のほうが
 * 鮮やかになってしまう。背景と比べれば、影は明るさが変わるだけで色みは変わらず、
 * 薄まったボールでも色みはずれる。
 *
 * 毎秒 120 フレームで呼ばれるので、作業用の [Mat] はフレームごとに作らず使い回す。
 * そのため同時に複数のスレッドから呼んではいけない。
 *
 * @param minColorShift 検出した領域の平均色が、背景の同じ場所の平均色から色みを変えた量の下限
 * (8 ビットの階調)。影や照明のような明るさだけの変化を捨てるのに使う。0 のときは色みで
 * 絞り込まない
 */
class ObjectTracker(
    val accumulateWeight: Double,
    val binaryThreshold: Double,
    val minContourArea: Double,
    val foregroundPersistenceFrames: Int = 15,
    val minColorShift: Double = 0.0
) {
    private var initialized = false
    private val background = Mat()
    private val foregroundAge = Mat()

    private val background8 = Mat()
    private val colorDelta = Mat()
    private val deltaChannel = Mat()
    private val frameDelta = Mat()
    private val threshold = Mat()
    private val thresholdOpen = Mat()
    private val thresholdClean = Mat()
    private val backgroundMask = Mat()
    private val persistentForegroundMask = Mat()
    private val backgroundUpdateMask = Mat()
    private val hierarchy = Mat()
    private val noHierarchy = Mat()
    private val kernel =
        Imgproc.getStructuringElement(Imgproc.MORPH_ELLIPSE, Size(5.0, 5.0), Point(-1.0, -1.0))

    init {
        require(foregroundPersistenceFrames > 0)
        require(minColorShift >= 0.0)
    }

    /**
     * @param frame 3 チャンネルのカラー画像 (CV_8UC3)。チャンネルの並びは問わない。書き換えない
     */
    fun nextFrame(frame: Mat): List<Rect> {
        if (!initialized || background.rows() != frame.rows() || background.cols() != frame.cols()
            || background.channels() != frame.channels()
        ) {
            frame.convertTo(background, CvType.CV_32F)
            Mat.zeros(frame.size(), CvType.CV_16UC1).copyTo(foregroundAge)
            initialized = true
            return emptyList()
        }

        Core.convertScaleAbs(background, background8)
        Core.absdiff(frame, background8, colorDelta)
        maxAcrossChannels(colorDelta, frameDelta)
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
        Imgproc.accumulateWeighted(frame, background, accumulateWeight, backgroundUpdateMask)

        val contours = mutableListOf<MatOfPoint>()
        Imgproc.findContours(
            thresholdClean,
            contours,
            hierarchy,
            Imgproc.RETR_EXTERNAL,
            Imgproc.CHAIN_APPROX_SIMPLE
        )

        val rects = contours.filter { Imgproc.contourArea(it) >= minContourArea }
            .map { it to Imgproc.boundingRect(it) }
            .filter { (contour, bbox) ->
                minColorShift <= 0.0 ||
                        colorShift(frame, background8, contour, bbox) >= minColorShift
            }
            .map { (_, bbox) -> bbox }
        contours.forEach { it.release() }
        return rects
    }

    private fun maxAcrossChannels(src: Mat, dst: Mat) {
        Core.extractChannel(src, dst, 0)
        for (channel in 1 until src.channels()) {
            Core.extractChannel(src, deltaChannel, channel)
            Core.max(dst, deltaChannel, dst)
        }
    }

    /**
     * 輪郭の内側の平均色が、背景の同じ場所の平均色から色みをどれだけ変えたかを返す
     * (8 ビットの階調)。
     *
     * 次の 2 つの残りの小さいほう。残りの大きさは、各チャンネルをベクトルとみたときの長さ。
     * 1. 明るさだけが何倍かになったとみなしたときの残り。影や照明の変化では、面の色に
     *    関係なく小さい。ただし 0 にはならず、録画では変化の大きさの 1〜3 割が残った
     *    (既知の影で 5.5〜7.4)。下限を下げるときは、影が通らないかを確かめること
     * 2. 全チャンネルが同じ量だけ増減したとみなしたときの残り。黒い背景に白い物体のような、
     *    色のないものどうしの変化ではほぼ 0
     *
     * @param background8 このフレームの差分に使った背景 (更新する前のものを 8 ビットにした画像)
     */
    private fun colorShift(frame: Mat, background8: Mat, contour: MatOfPoint, bbox: Rect): Double {
        val mask = Mat.zeros(bbox.size(), CvType.CV_8UC1)
        Imgproc.drawContours(
            mask,
            listOf(contour),
            0,
            Scalar(255.0),
            Imgproc.FILLED,
            Imgproc.LINE_8,
            noHierarchy,
            0,
            Point(-bbox.x.toDouble(), -bbox.y.toDouble())
        )
        val region = frame.submat(bbox)
        val now = Core.mean(region, mask).`val`.take(frame.channels())
        region.release()
        val backgroundRegion = background8.submat(bbox)
        val before = Core.mean(backgroundRegion, mask).`val`.take(frame.channels())
        backgroundRegion.release()
        mask.release()

        val dot = now.zip(before) { n, b -> n * b }.sum()
        val denominator = before.sumOf { it * it }
        val k = if (denominator > 0.0) dot / denominator else 0.0
        val scaleResidual = sqrt(now.zip(before) { n, b -> (n - k * b) * (n - k * b) }.sum())
        val delta = now.zip(before) { n, b -> n - b }
        val deltaMean = delta.average()
        val offsetResidual = sqrt(delta.sumOf { (it - deltaMean) * (it - deltaMean) })
        return min(scaleResidual, offsetResidual)
    }

    companion object {
        const val TAG = "ObjectTracker"
    }
}
