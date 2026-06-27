package com.taichi765.struckoutCameraApp

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.taichi765.struckoutCameraApp.camera.VideoEncoder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runTest
import org.junit.Test
import org.junit.runner.RunWith
import org.opencv.android.OpenCVLoader
import org.opencv.core.Mat
import org.opencv.imgproc.Imgproc
import org.opencv.videoio.VideoCapture
import timber.log.Timber
import java.nio.ByteBuffer
import kotlin.io.path.absolutePathString
import kotlin.io.path.createFile
import kotlin.io.path.exists
import kotlin.io.path.outputStream

@RunWith(AndroidJUnit4::class)
class VideoEncoderInstrumentedTest {

    @Test
    fun `VideoEncoder encodes properly`() = runTest {
        Timber.plant(Timber.DebugTree())
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

        var encoder: VideoEncoder? = null
        var job: Job? = null

        val frame = Mat()
        val yuv = Mat()
        while (cap.read(frame)) {
            Imgproc.cvtColor(frame, yuv, Imgproc.COLOR_BGR2YUV)
            if (encoder == null) {
                Timber.tag("Test").d("width:${frame.width()}")
                Timber.tag("Test").d("height: ${frame.height()}")
                encoder = VideoEncoder(inst.targetContext, frame.width(), frame.height())
                job = CoroutineScope(Dispatchers.Default).launch {
                    encoder.run()
                }
            }
            val bytes = ByteArray(yuv.total().toInt())
            encoder.writeFrame(bytes.size, 0, ByteBuffer.allocate(bytes.size).put(bytes))
        }

        job!!.join()
    }

    companion object {
        const val FILENAME = "sample.mp4"
    }
}