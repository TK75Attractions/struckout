package com.taichi765.struckoutCameraApp.network

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.withContext
import timber.log.Timber
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.net.Socket

class SynchronizerImpl : Synchronizer {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val state = MutableStateFlow<State>(State.DisConnected)
    override val isConnected = state.map { state ->
        state is State.Connected
    }.stateIn(
        scope = scope,
        started = SharingStarted.Eagerly,
        initialValue = false
    )

    override suspend fun connect() {
        withContext(Dispatchers.IO) {
            val socket = try {
                Socket(REMOTE_ADDRESS, REMOTE_PORT)
            } catch (e: IOException) {
                Timber.tag(TAG).w("failed to connect to sync server")
                throw e
            }
            Timber.tag(TAG).i("successfully established TCP connection between server")

            state.value = State.Connected(
                socket,
                output = socket.getOutputStream(),
                input = socket.getInputStream()
            )
        }
    }

    override fun getOutputStream(): OutputStream {
        val curState = state.value
        check(curState is State.Connected) {
            "TCP is not connected"
        }
        return curState.output
    }

    override fun getInputStream(): InputStream {
        val curState = state.value
        check(curState is State.Connected) {
            "TCP is not connected"
        }
        return curState.input
    }

    override fun close() {
        val curState = state.value
        if (curState !is State.Connected) {
            Timber.tag(TAG).d("close() is called, but it's not connected now")
            return
        }
        curState.socket.close()
        Timber.tag(TAG).d("successfully closed TCP socket")
    }

    object Factory : Synchronizer.Factory {
        override fun create(): Synchronizer {
            return SynchronizerImpl()
        }
    }

    private sealed interface State {
        data class Connected(
            val socket: Socket,
            val output: OutputStream,
            val input: InputStream
        ) : State

        object DisConnected : State
    }

    companion object {
        const val TAG = "SynchronizerImpl"
        const val REMOTE_ADDRESS = "192.168.10.110"
        const val REMOTE_PORT = 6262
    }
}