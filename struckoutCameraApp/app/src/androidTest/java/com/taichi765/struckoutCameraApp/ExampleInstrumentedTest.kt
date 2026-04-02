package com.taichi765.struckoutCameraApp

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.taichi765.struckoutCameraApp.imgproc.opencv.ObjectTracker
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.opencv.android.OpenCVLoader
import org.opencv.core.Mat
import org.opencv.videoio.VideoCapture
import kotlin.io.path.absolutePathString
import kotlin.io.path.createFile
import kotlin.io.path.exists
import kotlin.io.path.outputStream

/**
 * Instrumented test, which will execute on an Android device.
 *
 * See [testing documentation](http://d.android.com/tools/testing).
 */
@RunWith(AndroidJUnit4::class)
class ExampleInstrumentedTest {
    @Test
    fun useAppContext() {
        // Context of the app under test.
        val appContext = InstrumentationRegistry.getInstrumentation().targetContext
        assertEquals("com.taichi765.struckoutCameraApp", appContext.packageName)
    }

    @Test
    fun nextFrame_works() {
        OpenCVLoader.initLocal()
        val inst = InstrumentationRegistry.getInstrumentation()

        // calling packageがcom.taichi765.struckoutCameraAppなのでcom.taichi765.struckoutCameraApp.testのディレクトリにはアクセスできない
        val outPath =
            inst.targetContext.getExternalFilesDir(null)!!.toPath().resolve(FILENAME)
        // ここはtargetContextではなくcontextを使う
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


    companion object {
        const val TAG = "ObjectTrackerTest"
        const val FILENAME = "sample.mp4"
    }
}