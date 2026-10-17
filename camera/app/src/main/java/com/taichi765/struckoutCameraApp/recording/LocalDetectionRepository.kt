package com.taichi765.struckoutCameraApp.recording

import com.taichi765.struckoutCameraApp.network.types.DetectionData
import com.taichi765.struckoutCameraApp.proto.detectionsPacket
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

    suspend fun loadAll(): List<FrameEntity> = frameDao.loadAll()

    suspend fun deleteAll() = frameDao.deleteAll()

    companion object {
        const val TAG = "LocalDetectionRepository"
        const val DUMMY_CAMERA_ID = 99
    }
}