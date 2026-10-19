package com.taichi765.struckoutCameraApp.network

import androidx.annotation.CheckResult
import com.taichi765.struckoutCameraApp.recording.FrameEntity
import kotlinx.coroutines.flow.StateFlow
import java.io.Closeable
import java.io.IOException

interface LocalDetectionUploader : Closeable {
    val isConnected: StateFlow<Boolean>

    /**
     * @return `null` if connection succeeds.
     */
    @CheckResult
    suspend fun connect(): ConnectionError?

    /**
     * @return `null` if successfully uploaded.
     */
    @CheckResult
    suspend fun upload(frames: List<FrameEntity>): UploadError?

    // TODO: たぶんこれ消す
    interface Factory {
        fun create(): LocalDetectionUploader
    }

    /**
     * Error returned from [LocalDetectionUploader.connect].
     *
     * 今は一種類しか無いが将来的に拡張するかもしれないのでsealed interfaceにしておく.
     */
    sealed interface ConnectionError {
        data class TcpConnection(val cause: IOException) : ConnectionError
    }

    /**
     * Error returned from [LocalDetectionUploader.upload].
     */
    sealed interface UploadError {
        data object NotConnected : UploadError
        data class WriteFailed(val cause: IOException) : UploadError
        data class ReadFailed(val cause: IOException) : UploadError
        data class ServerError(val description: String) : UploadError

        /**
         * プロトコル実装の不備
         */
        data class ProtocolError(val description: String) : UploadError
    }
}