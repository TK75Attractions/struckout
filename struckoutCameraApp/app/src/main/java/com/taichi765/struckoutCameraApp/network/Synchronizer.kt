package com.taichi765.struckoutCameraApp.network

import kotlinx.coroutines.flow.StateFlow
import java.io.Closeable
import java.io.OutputStream

interface Synchronizer : Closeable {
    val isConnected: StateFlow<Boolean>
    suspend fun connect()
    fun getOutputStream(): OutputStream

    interface Factory {
        fun create(): Synchronizer
    }
}