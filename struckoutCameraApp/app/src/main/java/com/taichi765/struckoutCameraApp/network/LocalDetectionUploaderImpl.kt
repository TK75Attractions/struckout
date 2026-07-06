package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.network.LocalDetectionUploader.ConnectionError
import com.taichi765.struckoutCameraApp.network.LocalDetectionUploader.UploadError
import com.taichi765.struckoutCameraApp.proto.Struckout
import com.taichi765.struckoutCameraApp.recording.FrameEntity
import com.taichi765.struckoutCameraApp.recording.LocalDetectionRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.withContext
import timber.log.Timber
import tk75attractions.struckout.v1.XtaskSync
import tk75attractions.struckout.v1.XtaskSync.UploadResult.DataCase
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.net.Socket
import java.net.SocketException
import java.nio.ByteBuffer
import java.nio.ByteOrder
import javax.inject.Inject

class LocalDetectionUploaderImpl @Inject constructor() : LocalDetectionUploader {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val state = MutableStateFlow<State>(State.DisConnected)
    override val isConnected = state.map { state ->
        state is State.Connected
    }.stateIn(
        scope = scope,
        started = SharingStarted.Eagerly,
        initialValue = false
    )

    override suspend fun connect(): ConnectionError? {
        return withContext(Dispatchers.IO) {
            val socket = try {
                Socket(REMOTE_ADDRESS, REMOTE_PORT)
            } catch (e: IOException) {
                Timber.tag(TAG).w("failed to connect to sync server")
                return@withContext ConnectionError.TcpConnection(e)
            }
            Timber.tag(TAG).i("successfully established TCP connection between server")

            state.value = State.Connected(
                socket,
                output = socket.getOutputStream(),
                input = socket.getInputStream()
            )
            return@withContext null
        }
    }

    override suspend fun upload(frames: List<FrameEntity>): UploadError? {
        Timber.tag(LocalDetectionRepository.TAG).i("synchronizing local detections...")
        val curState = state.value
        if (curState !is State.Connected) {
            return UploadError.NotConnected
        }

        val total = frames.count()
        val header = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN)
            .putInt(total).array()
        try {
            withContext(Dispatchers.IO) {
                curState.output.write(header)
            }
        } catch (e: SocketException) {
            Timber.tag(TAG)
                .w("it seems TCP socket has been broken. you can recreate socket by calling connect() again.")
            state.value = State.DisConnected
            return UploadError.WriteFailed(e)
        } catch (e: IOException) {
            return UploadError.WriteFailed(e)
        }

        try {
            frames.forEach {
                val packet = Struckout.DetectionsPacket.newBuilder().mergeFrom(it.data).build()
                writePacket(curState.output, packet)
            }
        } catch (e: IOException) {
            return UploadError.WriteFailed(e)
        }

        val result = try {
            readPacket(curState.input, XtaskSync.UploadResult::parseFrom)
        } catch (e: IOException) {
            return UploadError.ReadFailed(e)
        }

        val error = when (result.dataCase) {
            DataCase.SUCCESS -> null
            DataCase.DB_CONNECTION_ERROR -> UploadError.ServerError(result.dbConnectionError)
            DataCase.DB_INSERT_ERROR -> UploadError.ServerError(result.dbInsertError)
            DataCase.TCP_ERROR -> UploadError.ServerError(result.tcpError)
            DataCase.PACKET_DECODE_ERROR -> UploadError.ProtocolError(result.packetDecodeError)
            DataCase.DATA_NOT_SET -> UploadError.ServerError("Received invalid packet")
        }
        if (error == null) {
            Timber.tag(LocalDetectionRepository.TAG).d("successfully synced all local frames")
        }
        return error
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

    object Factory : LocalDetectionUploader.Factory {
        override fun create(): LocalDetectionUploader {
            return LocalDetectionUploaderImpl()
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
        const val REMOTE_ADDRESS = "127.0.0.1"
        const val REMOTE_PORT = 6262
    }
}