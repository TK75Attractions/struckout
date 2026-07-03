package com.taichi765.struckoutCameraApp.camera

import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import org.opencv.core.Core
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Rect
import org.opencv.core.Scalar
import org.opencv.imgproc.Imgproc


class MyAnalyzer(
    val tracker: ObjectTracker,
    private val withAnalyzeResult: (AnalyzeResult) -> Unit,
) : ImageAnalysis.Analyzer {
    override fun analyze(image: ImageProxy) {
        image.use { image ->
            val timestamp = image.imageInfo.timestamp
            val mat = getMatFromImage(image)
            val rects = tracker.nextFrame(mat)

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
                    bboxes = rects
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

    data class AnalyzeResult(
        val mat: Mat,
        val imageTimestampMillis: Long,
        val bboxes: List<Rect>
    )


    companion object {
        const val TAG = "MyAnalyzer"
    }
}