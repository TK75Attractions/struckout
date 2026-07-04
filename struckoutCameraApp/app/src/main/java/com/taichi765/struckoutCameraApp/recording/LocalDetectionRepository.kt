package com.taichi765.struckoutCameraApp.recording

import com.taichi765.struckoutCameraApp.InputStreamCompat
import com.taichi765.struckoutCameraApp.network.bytesToInt
import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.network.writePacket
import com.taichi765.struckoutCameraApp.proto.Struckout
import com.taichi765.struckoutCameraApp.proto.detectionsPacket
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import timber.log.Timber
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import javax.inject.Inject
import kotlin.uuid.Uuid

/**
 * Saves detections to disk (in protobuf format).
 */
class LocalDetectionRepository @Inject constructor(private val frameDao: FrameDao) {
    val rowCount = frameDao.countRows()

    suspend fun pushDetection(data: DetectionData, sessionID: Uuid) {
        frameDao.insertFrame(
            FrameEntity(
                timestamp = data.timestamp,
                data = detectionsPacket {
                    cameraId = DUMMY_CAMERA_ID
                    sessionId = sessionID.toString()
                    timestamp = data.timestamp
                    frameId = data.frameId.toLong()
                    data.detections.forEach {
                        detections += it
                    }
                }
            )
        )
    }

    suspend fun syncAll(output: OutputStream, input: InputStream) {
        Timber.tag(TAG).i("synchronizing local detections...")
        val frames = frameDao.loadAll()

        val len = frames.count()
        val header = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN)
            .putInt(len).array()
        withContext(Dispatchers.IO) {
            output.write(header)
        }

        frames.forEach {
            val packet = Struckout.DetectionsPacket.newBuilder().mergeFrom(it.data).build()
            writePacket(output, packet)
        }

        withContext(Dispatchers.IO) {
            val retCode = bytesToInt(InputStreamCompat.readNBytes(input, len))
            if (retCode != 0) {
                throw IOException("failed to sync")
            }
        }
        Timber.tag(TAG).d("successfully synced all local frames")
    }

    suspend fun deleteAll() {
        frameDao.deleteAll()
    }

    companion object {
        const val TAG = "LocalDetectionRepository"
        const val DUMMY_CAMERA_ID = 99
    }
}