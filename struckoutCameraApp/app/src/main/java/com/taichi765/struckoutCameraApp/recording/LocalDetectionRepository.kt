package com.taichi765.struckoutCameraApp.recording

import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.network.writePacket
import com.taichi765.struckoutCameraApp.proto.Struckout
import com.taichi765.struckoutCameraApp.proto.udpPacket
import timber.log.Timber
import java.io.OutputStream
import javax.inject.Inject

/**
 * Saves detections to disk (in protobuf format).
 */
class LocalDetectionRepository @Inject constructor(private val frameDao: FrameDao) {
    val rowCount = frameDao.countRows()

    suspend fun pushDetection(data: DetectionData) {
        frameDao.insertFrame(
            FrameEntity(
                timestamp = data.timestamp,
                data = udpPacket {
                    cameraId = DUMMY_CAMERA_ID
                    timestamp = data.timestamp
                    frameId = data.frameId.toLong()
                    data.detections.forEach {
                        detectedObjects += it
                    }
                }
            )
        )
    }

    suspend fun syncAll(output: OutputStream) {
        frameDao.loadAll().forEach {
            val packet = Struckout.UdpPacket.newBuilder().mergeFrom(it.data).build()
            writePacket(output, packet)
        }
        Timber.tag(TAG).d("successfully synced all local frames")
    }

    companion object {
        const val TAG = "LocalDetectionRepository"
        const val DUMMY_CAMERA_ID = 99
    }
}