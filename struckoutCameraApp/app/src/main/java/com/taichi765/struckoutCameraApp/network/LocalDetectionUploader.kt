package com.taichi765.struckoutCameraApp.network

import kotlinx.coroutines.flow.StateFlow
import java.io.Closeable
import java.io.InputStream
import java.io.OutputStream

interface LocalDetectionUploader : Closeable {
    val isConnected: StateFlow<Boolean>
    suspend fun connect()
    fun getOutputStream(): OutputStream
    fun getInputStream(): InputStream

    interface Factory {
        fun create(): LocalDetectionUploader
    }
}