package com.taichi765.struckoutCameraApp.network

import java.io.Closeable


interface TcpSession : SessionStateProvider, Closeable {
    suspend fun connect(): Boolean

    interface Factory {
        fun create(): TcpSession
    }
}

