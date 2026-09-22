package com.taichi765.struckoutCameraApp

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.taichi765.struckoutCameraApp.camera.ObjectTracker
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.opencv.android.OpenCVLoader
import org.opencv.core.CvType
import org.opencv.core.Mat
import org.opencv.core.Point
import org.opencv.core.Scalar
import org.opencv.imgproc.Imgproc
import org.opencv.videoio.VideoCapture
import kotlin.io.path.absolutePathString
import kotlin.io.path.createFile
import kotlin.io.path.exists
import kotlin.io.path.outputStream

@RunWith(AndroidJUnit4::class)
class ObjectTrackerInstrumentedTest {

    @Test
    fun nextFrame_works() {
        OpenCVLoader.initLocal()
        val inst = InstrumentationRegistry.getInstrumentation()

        val outPath =
            inst.targetContext.getExternalFilesDir(null)!!.toPath().resolve(FILENAME)
        inst.context.assets.open(FILENAME).use { inputStream ->
            if (!outPath.exists()) {
                outPath.createFile()
            }
            outPath.outputStream().use { inputStream.copyTo(it) }
        }

        val cap = VideoCapture(
            outPath.absolutePathString(),
        )
        val tracker = ObjectTracker(0.5, 15.0, 80.0)

        val frame = Mat()
        while (cap.read(frame)) {
            tracker.nextFrame(frame)
        }
    }

    @Test
    fun movingObjectDoesNotLeaveGhostContours() {
        OpenCVLoader.initLocal()
        val tracker = ObjectTracker(0.5, 15.0, 80.0)
        val background = Mat.zeros(120, 160, CvType.CV_8UC3)

        assertTrue(tracker.nextFrame(background).isEmpty())

        val firstPosition = background.clone()
        Imgproc.rectangle(
            firstPosition,
            Point(20.0, 40.0),
            Point(40.0, 60.0),
            Scalar(255.0, 255.0, 255.0),
            Imgproc.FILLED
        )
        assertEquals(1, tracker.nextFrame(firstPosition).size)

        val secondPosition = background.clone()
        Imgproc.rectangle(
            secondPosition,
            Point(90.0, 40.0),
            Point(110.0, 60.0),
            Scalar(255.0, 255.0, 255.0),
            Imgproc.FILLED
        )
        val detections = tracker.nextFrame(secondPosition)

        assertEquals(1, detections.size)
        assertTrue(detections.single().x >= 85)
    }

    companion object {
        const val TAG = "ObjectTrackerTest"
        const val FILENAME = "sample.mp4"
    }
}
