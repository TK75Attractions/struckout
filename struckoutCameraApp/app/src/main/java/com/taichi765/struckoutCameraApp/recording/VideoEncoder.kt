package com.taichi765.struckoutCameraApp.recording

import android.content.ContentValues
import android.content.Context
import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaCodecList
import android.media.MediaFormat
import android.media.MediaMuxer
import android.provider.MediaStore
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import org.jetbrains.annotations.Blocking
import timber.log.Timber
import java.io.File
import java.nio.ByteBuffer
import kotlin.properties.Delegates

class VideoEncoder(val context: Context, width: Int, height: Int) {
    private var trackIdx by Delegates.notNull<Int>()
    private val runState = MutableStateFlow(RunState.Stopped)
    private val frameChannel = Channel<FrameData>(
        capacity = FRAME_CHANNEL_CAP,
        onBufferOverflow = BufferOverflow.DROP_OLDEST
    )

    private val codec = run {
        val format =
            MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AV1, width, height).apply {
                setInteger(MediaFormat.KEY_BIT_RATE, 4_000_000)
                setInteger(
                    MediaFormat.KEY_COLOR_FORMAT,
                    MediaCodecInfo.CodecCapabilities.COLOR_FormatYUV420Flexible
                )
                setInteger(MediaFormat.KEY_FRAME_RATE, 60)
                setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 0)
            }


        val name = MediaCodecList(MediaCodecList.REGULAR_CODECS).findEncoderForFormat(format)
        check(name != null) {
            "No encoder found for format $format"
        }
        Timber.tag(TAG).i(name)

        val cap = MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos.first {
            it.name == name
        }.getCapabilitiesForType(MediaFormat.MIMETYPE_VIDEO_AV1)
        Timber.tag(TAG).d("frameRate = ${cap.videoCapabilities!!.supportedFrameRates}")
        Timber.tag(TAG).d("width = ${cap.videoCapabilities!!.supportedWidths}")
        Timber.tag(TAG).d("height = ${cap.videoCapabilities!!.supportedHeights}")
        Timber.tag(TAG).d("bitrate = ${cap.videoCapabilities!!.bitrateRange}")
        Timber.tag(TAG).d("color format = ${cap.colorFormats.contentToString()}")

        runCatching {
            MediaCodec.createByCodecName(name).apply {
                configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
                start()
            }
        }.onFailure {
            Timber.tag(TAG).e("failed to initialize MediaCodec: $it")
        }.getOrThrow()
    }
    private val bufferInfo = MediaCodec.BufferInfo()
    private val tempFilePath =
        File.createTempFile("temp.mp4", null, context.cacheDir)

    private val muxer =
        MediaMuxer(tempFilePath.absolutePath, MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4).also {
            trackIdx = it.addTrack(codec.outputFormat)
        }


    /**
     * Loops until [stop] is called.
     */
    @Blocking
    suspend fun run() {
        while (runState.value == RunState.Running) {
            val inputIdx = codec.dequeueInputBuffer(DEQUEUE_INPUT_BUFFER_TIMEOUT)
            if (inputIdx < 0) {
                Timber.tag(TAG).d("no buffer available")
            }
            val inputBuf = codec.getInputBuffer(inputIdx)!!
            val frame = frameChannel.receive()
            inputBuf.put(frame.data)
            codec.queueInputBuffer(
                inputIdx,
                0,
                frame.size,
                frame.time,
                0
            )

            drainEncodedOutputs()
        }
        // TODO: END_OF_STREAM渡す
    }

    private fun drainEncodedOutputs() {
        while (true) {
            val outputIdx = codec.dequeueOutputBuffer(bufferInfo, DEQUEUE_OUTPUT_BUFFER_TIMEOUT)
            if (outputIdx < 0) {
                break
            }
            val outputBuf = codec.getOutputBuffer(outputIdx)!!
            muxer.writeSampleData(outputIdx, outputBuf, bufferInfo)
            codec.releaseOutputBuffer(outputIdx, false)
        }
    }

    fun stop() {
        Timber.tag(TAG).i("stopping VideoEncoder")
        runState.value = RunState.Stopped
    }

    suspend fun writeFrame(size: Int, time: Long, frame: ByteBuffer) {
        frameChannel.send(FrameData(size, time, frame))
    }

    private fun flashToMediaStore() {
        Timber.tag(TAG).d("flashing recoded video to MediaStore")
        val resolver = context.contentResolver
        val values = ContentValues().apply {
            put(MediaStore.Video.Media.DISPLAY_NAME, "my_video")
            put(MediaStore.Video.Media.MIME_TYPE, "video/mp4")
        }
        val uri =
            resolver.insert(MediaStore.Video.Media.EXTERNAL_CONTENT_URI, values)
                ?: throw IllegalStateException("failed to insert to MediaStore")
        resolver.openOutputStream(uri).use {
        }
    }


    private data class FrameData(val size: Int, val time: Long, val data: ByteBuffer)

    private sealed interface RunState {
        object Running : RunState
        object Stopped : RunState
    }

    companion object {
        const val TAG = "VideoEncoder"
        const val DEQUEUE_INPUT_BUFFER_TIMEOUT: Long = 1_000_000
        const val DEQUEUE_OUTPUT_BUFFER_TIMEOUT: Long = 1_000_000
        const val FRAME_CHANNEL_CAP = 5
    }
}