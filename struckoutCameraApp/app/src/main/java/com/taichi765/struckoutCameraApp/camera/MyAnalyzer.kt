package com.taichi765.struckoutCameraApp.camera

import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import org.opencv.core.Core
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Point
import org.opencv.core.Rect
import org.opencv.core.Scalar
import org.opencv.imgproc.Imgproc
import java.util.concurrent.TimeUnit


/**
 * @param detectionArea 検出を受け付ける範囲。フレームごとに読むので、設定の変更はすぐ反映される
 * @param onRawFrame 検出の枠を描き込む前の解析画像 (センサーの向きのまま) を受け取る。
 * 画像はこの呼び出しのあとで書き換えられるので、保持する場合はコピーすること
 */
class MyAnalyzer(
    val tracker: ObjectTracker,
    private val detectionArea: () -> DetectionArea,
    private val onRawFrame: (Mat) -> Unit,
    private val withAnalyzeResult: (AnalyzeResult) -> Unit,
) : ImageAnalysis.Analyzer {
    override fun analyze(image: ImageProxy) {
        image.use { image ->
            val timestampMillis = TimeUnit.NANOSECONDS.toMillis(image.imageInfo.timestamp)
            val mat = getMatFromImage(image)
            val area = detectionArea()
            val (rects, ignored) = tracker.nextFrame(mat).partition {
                area.contains(it.x + it.width / 2.0, it.y + it.height / 2.0, mat.cols(), mat.rows())
            }
            onRawFrame(mat)

            // 範囲を合わせやすいよう、範囲の枠と、範囲の外で捨てた検出もプレビューに描く。
            if (!area.coversWholeImage) {
                val bounds = area.inAnalysisImage(mat.cols(), mat.rows())
                Imgproc.rectangle(
                    mat,
                    Point(bounds.left, bounds.top),
                    Point(bounds.right, bounds.bottom),
                    Scalar(0.0, 255.0, 0.0)
                )
            }
            ignored.forEach {
                Imgproc.rectangle(
                    mat,
                    it,
                    Scalar(128.0, 128.0, 128.0)
                )
            }
            rects.forEach {
                Imgproc.rectangle(
                    mat,
                    it,
                    Scalar(255.0, 0.0, 0.0)
                )
            }

            val rotated = Mat().let {
                Core.rotate(mat, it, Core.ROTATE_90_CLOCKWISE)
                it
            }
            withAnalyzeResult(
                AnalyzeResult(
                    mat = rotated,
                    imageTimestampMillis = timestampMillis,
                    imageTimestampNanos = image.imageInfo.timestamp,
                    bboxes = rects,
                    imageWidth = mat.cols(),
                    imageHeight = mat.rows()
                )
            )
        }
    }

    private fun getMatFromImage(image: ImageProxy): Mat {
        /* https://stackoverflow.com/questions/30510928/convert-android-camera2-api-yuv-420-888-to-rgb */
        val yBuffer = image.planes[0].buffer
        val uBuffer = image.planes[1].buffer
        val vBuffer = image.planes[2].buffer
        val ySize = yBuffer.remaining()
        val uSize = uBuffer.remaining()
        val vSize = vBuffer.remaining()
        val nv21 = ByteArray(ySize + uSize + vSize)
        yBuffer.get(nv21, 0, ySize)
        vBuffer.get(nv21, ySize, vSize)
        uBuffer.get(nv21, ySize + vSize, uSize)
        val yuv = Mat(image.height + image.height / 2, image.width, CvType.CV_8UC1)
        yuv.put(0, 0, nv21)
        val mat = Mat()
        Imgproc.cvtColor(yuv, mat, Imgproc.COLOR_YUV2RGB_NV21, 3)
        return mat
    }

    /**
     * @param mat プレビュー用に回転させた画像
     * @param imageTimestampNanos フレームのセンサー時刻。カメラが付けた値そのまま
     * @param bboxes 回転前の画像での検出位置
     * @param imageWidth,imageHeight 回転前の画像の大きさ
     */
    data class AnalyzeResult(
        val mat: Mat,
        val imageTimestampMillis: Long,
        val imageTimestampNanos: Long,
        val bboxes: List<Rect>,
        val imageWidth: Int,
        val imageHeight: Int
    )


    companion object {
        const val TAG = "MyAnalyzer"
    }
}