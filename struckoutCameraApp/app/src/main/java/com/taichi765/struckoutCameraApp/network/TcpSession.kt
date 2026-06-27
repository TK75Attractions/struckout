package com.taichi765.struckoutCameraApp.network

import kotlinx.coroutines.flow.StateFlow
import java.io.Closeable


interface TcpSession : SessionStateProvider, Closeable {
    suspend fun connect(): Boolean

    interface Factory {
        fun create(): TcpSession
    }
}

/**
 * Provides information about session state (e.g. camera id given by TCP server).
 */
interface SessionStateProvider {
    val state: StateFlow<SessionState>
}

sealed interface SessionState {
    data class Connected(val cameraID: UInt) : SessionState

    object DisConnected : SessionState
}

