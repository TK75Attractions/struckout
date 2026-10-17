package com.taichi765.struckoutCameraApp.camera.highspeed

import android.graphics.SurfaceTexture
import android.opengl.EGL14
import android.opengl.EGLConfig
import android.opengl.EGLContext
import android.opengl.EGLDisplay
import android.opengl.EGLSurface
import android.opengl.GLES11Ext
import android.opengl.GLES20
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.FloatBuffer
import kotlin.math.max
import kotlin.math.min

/**
 * [SurfaceTexture] に届いたフレームを、[width] x [height] に縮小して RGBA で読み出す。
 *
 * 高速撮影セッションは YUV の ImageReader を出力先にできないので、カメラのフレームは動画エンコーダーと
 * デコーダー ([VideoRelay]) を通してこの SurfaceTexture に描かれる。GPU で縮小してから CPU 側へ持ってくる。
 *
 * すべてのメソッドは、[create] を呼んだスレッドから呼ぶこと (EGL コンテキストがそのスレッドに属する)。
 */
class GlFrameReader private constructor(
    val width: Int,
    val height: Int,
    private val display: EGLDisplay,
    private val context: EGLContext,
    private val pbuffer: EGLSurface,
) {
    private val oesTexture: Int
    private val program: Int
    private val framebuffer: Int
    private val colorTexture: Int
    private val positionLocation: Int
    private val rectLocation: Int
    private val transform = FloatArray(16)

    private val quad: FloatBuffer = ByteBuffer.allocateDirect(8 * 4)
        .order(ByteOrder.nativeOrder())
        .asFloatBuffer()
        .apply {
            put(floatArrayOf(-1f, -1f, 1f, -1f, -1f, 1f, 1f, 1f))
            position(0)
        }

    val surfaceTexture: SurfaceTexture

    init {
        val ids = IntArray(1)

        GLES20.glGenTextures(1, ids, 0)
        oesTexture = ids[0]
        GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, oesTexture)
        setLinearClamp(GLES11Ext.GL_TEXTURE_EXTERNAL_OES)

        GLES20.glGenTextures(1, ids, 0)
        colorTexture = ids[0]
        GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, colorTexture)
        GLES20.glTexImage2D(
            GLES20.GL_TEXTURE_2D, 0, GLES20.GL_RGBA, width, height, 0,
            GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, null
        )
        setLinearClamp(GLES20.GL_TEXTURE_2D)

        GLES20.glGenFramebuffers(1, ids, 0)
        framebuffer = ids[0]
        GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER, framebuffer)
        GLES20.glFramebufferTexture2D(
            GLES20.GL_FRAMEBUFFER, GLES20.GL_COLOR_ATTACHMENT0,
            GLES20.GL_TEXTURE_2D, colorTexture, 0
        )
        check(GLES20.glCheckFramebufferStatus(GLES20.GL_FRAMEBUFFER) == GLES20.GL_FRAMEBUFFER_COMPLETE) {
            "framebuffer is incomplete"
        }

        program = linkProgram(VERTEX_SHADER, FRAGMENT_SHADER)
        positionLocation = GLES20.glGetAttribLocation(program, "aPosition")
        rectLocation = GLES20.glGetUniformLocation(program, "uRect")

        GLES20.glViewport(0, 0, width, height)
        GLES20.glPixelStorei(GLES20.GL_PACK_ALIGNMENT, 1)

        surfaceTexture = SurfaceTexture(oesTexture)
    }

    /**
     * 次のフレームを取り込む。読み出さない場合もフレームごとに必ず呼ぶこと。
     * 呼ばないとキューが詰まり、デコーダー側が止まる。
     *
     * @return フレームの時刻 (ナノ秒)。デコーダーが付けた時刻で、マイクロ秒に丸められている。
     * センサー時刻に戻すのは [HighSpeedCamera] の仕事
     */
    fun acquireFrame(): Long {
        surfaceTexture.updateTexImage()
        return surfaceTexture.timestamp
    }

    /**
     * 直前に [acquireFrame] したフレームを [dst] に RGBA で書き込む。
     *
     * 行は画像の上から順に並ぶ。向きはセンサーの出力そのまま (端末の向きに合わせた回転はしない)。
     * ImageAnalysis の画像と同じ向きなので、検出位置から方向ベクトルを求める計算をそのまま使える。
     *
     * @param dst 容量 [width] * [height] * 4 以上のダイレクトバッファ
     */
    fun readFrame(dst: ByteBuffer) {
        surfaceTexture.getTransformMatrix(transform)
        val rect = validTextureRect(transform)

        GLES20.glUseProgram(program)
        GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
        GLES20.glBindTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES, oesTexture)
        GLES20.glUniform4f(rectLocation, rect[0], rect[1], rect[2], rect[3])
        GLES20.glEnableVertexAttribArray(positionLocation)
        GLES20.glVertexAttribPointer(positionLocation, 2, GLES20.GL_FLOAT, false, 0, quad)
        GLES20.glDrawArrays(GLES20.GL_TRIANGLE_STRIP, 0, 4)

        dst.clear()
        GLES20.glReadPixels(0, 0, width, height, GLES20.GL_RGBA, GLES20.GL_UNSIGNED_BYTE, dst)
    }

    fun release() {
        surfaceTexture.release()
        GLES20.glDeleteProgram(program)
        GLES20.glDeleteFramebuffers(1, intArrayOf(framebuffer), 0)
        GLES20.glDeleteTextures(2, intArrayOf(oesTexture, colorTexture), 0)
        EGL14.eglMakeCurrent(
            display, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_SURFACE, EGL14.EGL_NO_CONTEXT
        )
        EGL14.eglDestroySurface(display, pbuffer)
        EGL14.eglDestroyContext(display, context)
        EGL14.eglTerminate(display)
    }

    private fun setLinearClamp(target: Int) {
        GLES20.glTexParameteri(target, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR)
        GLES20.glTexParameteri(target, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR)
        GLES20.glTexParameteri(target, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE)
        GLES20.glTexParameteri(target, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE)
    }

    companion object {
        // 画面の下端 (y = -1) をテクスチャの v の小さい側に対応させる。glReadPixels は下の行から
        // 読むので、これで読み出したバッファの先頭が画像の上端になる。
        private const val VERTEX_SHADER = """
            attribute vec2 aPosition;
            uniform vec4 uRect;
            varying vec2 vTexCoord;
            void main() {
                gl_Position = vec4(aPosition, 0.0, 1.0);
                vTexCoord = uRect.xy + (aPosition + 1.0) * 0.5 * uRect.zw;
            }
        """

        private const val FRAGMENT_SHADER = """
            #extension GL_OES_EGL_image_external : require
            precision mediump float;
            uniform samplerExternalOES uTexture;
            varying vec2 vTexCoord;
            void main() {
                gl_FragColor = texture2D(uTexture, vTexCoord);
            }
        """

        /**
         * 呼び出したスレッドに EGL コンテキストを作り、[width] x [height] で読み出す準備をする。
         */
        fun create(width: Int, height: Int): GlFrameReader {
            require(width > 0 && height > 0) { "size must be positive" }

            val display = EGL14.eglGetDisplay(EGL14.EGL_DEFAULT_DISPLAY)
            check(display != EGL14.EGL_NO_DISPLAY) { "no EGL display" }
            val version = IntArray(2)
            check(EGL14.eglInitialize(display, version, 0, version, 1)) { "eglInitialize failed" }

            val configs = arrayOfNulls<EGLConfig>(1)
            val configCount = IntArray(1)
            val configAttributes = intArrayOf(
                EGL14.EGL_RENDERABLE_TYPE, EGL14.EGL_OPENGL_ES2_BIT,
                EGL14.EGL_SURFACE_TYPE, EGL14.EGL_PBUFFER_BIT,
                EGL14.EGL_RED_SIZE, 8,
                EGL14.EGL_GREEN_SIZE, 8,
                EGL14.EGL_BLUE_SIZE, 8,
                EGL14.EGL_ALPHA_SIZE, 8,
                EGL14.EGL_NONE
            )
            check(
                EGL14.eglChooseConfig(display, configAttributes, 0, configs, 0, 1, configCount, 0)
                        && configCount[0] > 0
            ) { "no suitable EGL config" }
            val config = configs[0]

            val context = EGL14.eglCreateContext(
                display, config, EGL14.EGL_NO_CONTEXT,
                intArrayOf(EGL14.EGL_CONTEXT_CLIENT_VERSION, 2, EGL14.EGL_NONE), 0
            )
            check(context != EGL14.EGL_NO_CONTEXT) { "eglCreateContext failed" }

            // 描画先は自前のフレームバッファなので、ここは形だけあればよい。
            val pbuffer = EGL14.eglCreatePbufferSurface(
                display, config,
                intArrayOf(EGL14.EGL_WIDTH, 1, EGL14.EGL_HEIGHT, 1, EGL14.EGL_NONE), 0
            )
            check(pbuffer != EGL14.EGL_NO_SURFACE) { "eglCreatePbufferSurface failed" }
            check(EGL14.eglMakeCurrent(display, pbuffer, pbuffer, context)) { "eglMakeCurrent failed" }

            return GlFrameReader(width, height, display, context, pbuffer)
        }

        /**
         * [SurfaceTexture.getTransformMatrix] の行列から、バッファのうち画像が入っている範囲を
         * テクスチャ座標 `(u, v, 幅, 高さ)` で返す。
         *
         * 行列には端末の向きに合わせた回転や上下反転も入っているが、それらは使わない。
         * 四隅の行き先の最小・最大だけを取ることで、切り出し範囲 (バッファ端の余白の除去) だけを反映する。
         */
        internal fun validTextureRect(transform: FloatArray): FloatArray {
            require(transform.size == 16) { "transform must be a 4x4 matrix" }
            var minU = Float.MAX_VALUE
            var minV = Float.MAX_VALUE
            var maxU = -Float.MAX_VALUE
            var maxV = -Float.MAX_VALUE
            for (s in 0..1) {
                for (t in 0..1) {
                    // 列優先の 4x4 行列に (s, t, 0, 1) を掛ける。
                    val u = transform[0] * s + transform[4] * t + transform[12]
                    val v = transform[1] * s + transform[5] * t + transform[13]
                    minU = min(minU, u)
                    maxU = max(maxU, u)
                    minV = min(minV, v)
                    maxV = max(maxV, v)
                }
            }
            return floatArrayOf(minU, minV, maxU - minU, maxV - minV)
        }

        private fun linkProgram(vertexSource: String, fragmentSource: String): Int {
            val vertex = compileShader(GLES20.GL_VERTEX_SHADER, vertexSource)
            val fragment = compileShader(GLES20.GL_FRAGMENT_SHADER, fragmentSource)
            val program = GLES20.glCreateProgram()
            GLES20.glAttachShader(program, vertex)
            GLES20.glAttachShader(program, fragment)
            GLES20.glLinkProgram(program)
            val status = IntArray(1)
            GLES20.glGetProgramiv(program, GLES20.GL_LINK_STATUS, status, 0)
            check(status[0] == GLES20.GL_TRUE) {
                "failed to link program: ${GLES20.glGetProgramInfoLog(program)}"
            }
            GLES20.glDeleteShader(vertex)
            GLES20.glDeleteShader(fragment)
            return program
        }

        private fun compileShader(type: Int, source: String): Int {
            val shader = GLES20.glCreateShader(type)
            GLES20.glShaderSource(shader, source)
            GLES20.glCompileShader(shader)
            val status = IntArray(1)
            GLES20.glGetShaderiv(shader, GLES20.GL_COMPILE_STATUS, status, 0)
            check(status[0] == GLES20.GL_TRUE) {
                "failed to compile shader: ${GLES20.glGetShaderInfoLog(shader)}"
            }
            return shader
        }
    }
}
