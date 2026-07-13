package com.taichi765.struckoutCameraApp.network

import androidx.annotation.CheckResult
import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.flow.StateFlow
import java.io.IOException

interface DataConnection {
    val isConnected: StateFlow<Boolean>

    /**
     * @return `null` if connection succeeds.
     */
    @CheckResult
    suspend fun connect(): DataConnectionError?
    suspend fun sendPacket(packet: Struckout.DetectionsPacket)

    interface Factory {
        fun create(): DataConnection
    }
}

/**
 * Error returned from [DataConnection.connect].
 *
 * 今は一種類しか無いが後で拡張するかもしれないのでsealed interfaceにしておく.
 */
sealed interface DataConnectionError {
    data class TcpConnectionFailed(val cause: IOException) : DataConnectionError
}