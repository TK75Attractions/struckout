package com.taichi765.struckoutCameraApp.network

import kotlinx.coroutines.flow.StateFlow
import java.io.Closeable
import java.io.IOException


interface TcpSession : SessionStateProvider, Closeable {
    suspend fun connect(): ConnectionError?

    interface Factory {
        fun create(): TcpSession
    }

    sealed interface ConnectionError {
        data class TcpConnectionFailed(val cause: IOException) : ConnectionError

        data class InitializationFailed(val reason: InitializationError) : ConnectionError

        sealed interface InitializationError {
            data class ReadPacketFailed(val cause: IOException) : InitializationError

            data object InvalidPacket : InitializationError
        }
    }
}

/**
 * Provides information about session state (e.g. camera id given by TCP server).
 */
interface SessionStateProvider {
    val state: StateFlow<SessionState>
}

/**
 * Used in [TcpSession] and [SessionStateProvider].
 */
sealed interface SessionState {
    data class Connected(val cameraID: UInt) : SessionState

    data object DisConnected : SessionState
}
