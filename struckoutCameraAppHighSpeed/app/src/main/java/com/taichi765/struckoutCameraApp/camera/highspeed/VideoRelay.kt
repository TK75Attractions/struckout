package com.taichi765.struckoutCameraApp.camera.highspeed

import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.util.Size
import android.view.Surface
import timber.log.Timber
import java.util.ArrayDeque
import java.util.concurrent.atomic.AtomicInteger

/**
 * カメラのフレームを動画エンコーダーに通し、すぐにデコードして [output] に描く。
 *
 * 高速撮影セッションでは、プレビュー用の出力先 (SurfaceTexture など) に 30fps しか渡さない端末がある。
 * KYV45 では、センサーは 120fps で動いているのに、SurfaceTexture に届くのは 4 枚に 1 枚だった
 * (届いたフレームの間隔がすべて 1/120 秒のちょうど 4 倍)。動画エンコーダーの入力は高速撮影の本来の
 * 出力先なので、120fps のまま届く見込み。代わりに、エンコードとデコードのぶん遅れ (数フレーム) と
 * 端末の負荷が増える。ファイルには書かない。
 *
 * [onEncoded] を渡すと、エンコーダーが出したデータをそのまま受け取れる (PC で解析してもらうとき)。
 * 引数はデータ (H.264、Annex B)、時刻 (マイクロ秒、エンコーダーが付けた値)、キーフレームか、設定 (SPS/PPS) か。
 *
 * コールバックはすべて [handler] のスレッドで呼ばれる。[release] と [requestKeyFrame] も同じスレッドから呼ぶこと。
 */
class VideoRelay(
    private val size: Size,
    private val frameRate: Int,
    private val output: Surface,
    private val handler: Handler,
    private val onEncoded: ((ByteArray, Long, Boolean, Boolean) -> Unit)? = null,
    private val onError: (String, Throwable?) -> Unit,
) {
    /** エンコーダーが出力してデコーダーに渡す前の、1 フレーム分のデータ。 */
    private class Packet(val data: ByteArray, val presentationTimeUs: Long)

    // handler のスレッドからのみ触る
    private val packets = ArrayDeque<Packet>()
    private val decoderInputs = ArrayDeque<Int>()
    private var decoder: MediaCodec? = null

    // デコーダーはキーフレームからしか始められない。詰まって捨てたあとも、次のキーフレームを待つ
    private var waitingForKeyFrame = true

    // 直前のフレームを、キーフレームだけ渡す決まりで扱ったか (handler のスレッドからのみ触る)
    private var keyFramesOnlyBefore = false

    /**
     * true のあいだ、デコーダーにはキーフレーム (1 秒に 1 枚) だけを渡す。PC が解析を引き受けていて、
     * この端末ではデコードした画像を画面の更新にしか使わないとき。デコーダーとエンコーダーは同じ回路を
     * 取り合うので、デコードを減らすとエンコーダーが 120fps に追いつきやすくなる (追いつかないと、
     * エンコーダーの入力に 20 枚ほどたまって 160〜200 ms 遅れ、あふれたフレームが抜けるとみている。2026-10-16 の実機)。
     */
    @Volatile
    var keyFramesOnly = false

    @Volatile
    private var released = false

    /** エンコーダーが出力したフレームの数。集計用で、読み手が 0 に戻す。 */
    val encodedFrames = AtomicInteger()

    private val encoderCallback = object : MediaCodec.Callback() {
        override fun onInputBufferAvailable(codec: MediaCodec, index: Int) = Unit // 入力は Surface から

        override fun onOutputBufferAvailable(codec: MediaCodec, index: Int, info: MediaCodec.BufferInfo) {
            if (released) return
            try {
                val config = info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG != 0
                val keyFrame = info.flags and MediaCodec.BUFFER_FLAG_KEY_FRAME != 0
                val data = if (info.size > 0 && (onEncoded != null || !config)) {
                    val buffer = checkNotNull(codec.getOutputBuffer(index))
                    buffer.position(info.offset)
                    buffer.limit(info.offset + info.size)
                    ByteArray(info.size).also { buffer.get(it) }
                } else {
                    null
                }
                if (data != null) {
                    onEncoded?.invoke(data, info.presentationTimeUs, keyFrame, config)
                }
                // 設定 (SPS/PPS) はデコーダーの設定に入れるので、データとしては渡さない
                if (!config && data != null) {
                    encodedFrames.incrementAndGet()
                    if (packets.size >= MAX_PENDING_PACKETS) {
                        // デコーダーが追いつかない。途中から渡すと壊れた画像になるので、次のキーフレームまで捨てる
                        Timber.tag(TAG).w("decoder is falling behind; dropping %d frames", packets.size)
                        packets.clear()
                        waitingForKeyFrame = true
                    }
                    val keyOnly = keyFramesOnly
                    if (keyOnly) {
                        if (keyFrame) {
                            packets.add(Packet(data, info.presentationTimeUs))
                        }
                    } else {
                        // キーフレームだけ渡していたあとは、次のキーフレームまで待つ (間のフレームが参照する画像がない)
                        if (keyFramesOnlyBefore) {
                            waitingForKeyFrame = true
                        }
                        if (keyFrame || !waitingForKeyFrame) {
                            waitingForKeyFrame = false
                            packets.add(Packet(data, info.presentationTimeUs))
                        }
                    }
                    keyFramesOnlyBefore = keyOnly
                }
                codec.releaseOutputBuffer(index, false)
                feedDecoder()
            } catch (e: Exception) {
                fail("failed to pass an encoded frame", e)
            }
        }

        override fun onOutputFormatChanged(codec: MediaCodec, format: MediaFormat) {
            if (released || decoder != null) return
            try {
                startDecoder(format)
            } catch (e: Exception) {
                fail("failed to start the video decoder", e)
            }
        }

        override fun onError(codec: MediaCodec, e: MediaCodec.CodecException) {
            fail("video encoder error", e)
        }
    }

    private val decoderCallback = object : MediaCodec.Callback() {
        override fun onInputBufferAvailable(codec: MediaCodec, index: Int) {
            if (released) return
            decoderInputs.add(index)
            try {
                feedDecoder()
            } catch (e: Exception) {
                fail("failed to decode a frame", e)
            }
        }

        override fun onOutputBufferAvailable(codec: MediaCodec, index: Int, info: MediaCodec.BufferInfo) {
            if (released) return
            try {
                // 描くと、output の SurfaceTexture にフレームが届く。時刻は presentationTimeUs * 1000 になる
                codec.releaseOutputBuffer(index, info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM == 0)
            } catch (e: Exception) {
                fail("failed to render a decoded frame", e)
            }
        }

        override fun onOutputFormatChanged(codec: MediaCodec, format: MediaFormat) = Unit

        override fun onError(codec: MediaCodec, e: MediaCodec.CodecException) {
            fail("video decoder error", e)
        }
    }

    private val encoder: MediaCodec = MediaCodec.createEncoderByType(MIME_TYPE)

    /** カメラの出力先にする Surface。 */
    val inputSurface: Surface

    init {
        val format = MediaFormat.createVideoFormat(MIME_TYPE, size.width, size.height).apply {
            setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface)
            setInteger(MediaFormat.KEY_BIT_RATE, (size.width * size.height * frameRate * BITS_PER_PIXEL).toInt())
            setInteger(MediaFormat.KEY_FRAME_RATE, frameRate)
            setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, I_FRAME_INTERVAL_SECONDS)
            // 実時間で処理するよう頼む (0 が実時間)
            setInteger(MediaFormat.KEY_PRIORITY, 0)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                // B フレームがあると、フレームが出てくるのが遅れる
                setInteger(MediaFormat.KEY_MAX_B_FRAMES, 0)
                // キーフレームのたびに SPS/PPS を付ける。途中から受け取る相手 (PC) もデコードを始められる
                setInteger(MediaFormat.KEY_PREPEND_HEADER_TO_SYNC_FRAMES, 1)
            }
        }
        inputSurface = try {
            startEncoder(format)
        } catch (e: Exception) {
            encoder.release()
            throw e
        }
        Timber.tag(TAG).i("video encoder started: %s at %dfps (%s)", size, frameRate, encoder.name)
    }

    /**
     * 実時間で撮影の fps をこなすよう頼んで (KEY_OPERATING_RATE)、エンコーダーを始める。
     * KYV45 では、これより大きい値を頼むと動かなくなる (2026-10-16):
     * - Short.MAX_VALUE: configure は通るが、createInputSurface が 0xfffffc0e で失敗する
     * - 撮影の 2 倍 (240): 始まるが、同じ回路を使うデコーダーが動かなくなる (OMX-VDEC の Failed to qbuf)
     */
    private fun startEncoder(format: MediaFormat): Surface {
        val rates = listOf(frameRate)
        for (rate in rates) {
            format.setInteger(MediaFormat.KEY_OPERATING_RATE, rate)
            var surface: Surface? = null
            try {
                encoder.setCallback(encoderCallback, handler)
                encoder.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
                surface = encoder.createInputSurface()
                encoder.start()
                Timber.tag(TAG).i("video encoder operating rate: %d", rate)
                return surface
            } catch (e: Exception) {
                surface?.release()
                if (rate == rates.last()) throw e
                Timber.tag(TAG).w(e, "the encoder rejected operating rate %d; retrying with the next rate", rate)
                encoder.reset()
            }
        }
        error("unreachable")
    }

    /** 次のフレームをキーフレームにするよう頼む。 */
    fun requestKeyFrame() {
        if (released) return
        runCatching {
            encoder.setParameters(Bundle().apply { putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0) })
        }.onFailure { Timber.tag(TAG).w(it, "failed to request a key frame") }
    }

    fun release() {
        released = true
        runCatching { encoder.stop() }
        runCatching { encoder.release() }
        decoder?.let {
            runCatching { it.stop() }
            runCatching { it.release() }
        }
        decoder = null
        inputSurface.release()
        packets.clear()
        decoderInputs.clear()
    }

    private fun startDecoder(encoderFormat: MediaFormat) {
        val format = MediaFormat.createVideoFormat(MIME_TYPE, size.width, size.height).apply {
            for (key in CODEC_CONFIG_KEYS) {
                if (encoderFormat.containsKey(key)) setByteBuffer(key, encoderFormat.getByteBuffer(key))
            }
            // キーフレームは平均の 10 倍ほどになる。小さいと入力に入りきらない
            setInteger(MediaFormat.KEY_MAX_INPUT_SIZE, size.width * size.height)
            setInteger(MediaFormat.KEY_OPERATING_RATE, frameRate)
            setInteger(MediaFormat.KEY_PRIORITY, 0)
        }
        val codec = MediaCodec.createDecoderByType(MIME_TYPE)
        try {
            codec.setCallback(decoderCallback, handler)
            codec.configure(format, output, null, 0)
            codec.start()
        } catch (e: Exception) {
            codec.release()
            throw e
        }
        decoder = codec
        Timber.tag(TAG).i("video decoder started (%s)", codec.name)
    }

    private fun feedDecoder() {
        val codec = decoder ?: return
        while (packets.isNotEmpty() && decoderInputs.isNotEmpty()) {
            val index = decoderInputs.poll()!!
            val packet = packets.poll()!!
            val buffer = checkNotNull(codec.getInputBuffer(index))
            buffer.clear()
            if (packet.data.size > buffer.capacity()) {
                // 入りきらないフレームは渡せない。次のキーフレームから渡し直す
                Timber.tag(TAG).w("encoded frame (%d bytes) does not fit the decoder input", packet.data.size)
                codec.queueInputBuffer(index, 0, 0, packet.presentationTimeUs, 0)
                packets.clear()
                waitingForKeyFrame = true
                return
            }
            buffer.put(packet.data)
            codec.queueInputBuffer(index, 0, packet.data.size, packet.presentationTimeUs, 0)
        }
    }

    private fun fail(message: String, cause: Throwable?) {
        if (released) return
        onError(message, cause)
    }

    companion object {
        private const val TAG = "VideoRelay"
        private const val MIME_TYPE = MediaFormat.MIMETYPE_VIDEO_AVC

        /** 1 画素 1 フレームあたりのビット数。1280x720、120fps で約 28Mbps。色の判定が圧縮で崩れないよう高めにする */
        private const val BITS_PER_PIXEL = 0.25
        private const val I_FRAME_INTERVAL_SECONDS = 1

        /** デコーダーに渡す前にためておけるフレームの数 (120fps で約 0.5 秒)。これを超えたら捨てる */
        private const val MAX_PENDING_PACKETS = 60
        private val CODEC_CONFIG_KEYS = listOf("csd-0", "csd-1")
    }
}
