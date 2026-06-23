package com.taichi765.struckoutCameraApp.transport

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

/**
 * Provides session infos given by server.
 */
class SessionInfoRepository(tcpTransport: TcpTransport) {
    val cameraID: Flow<UInt> = tcpTransport.state.map { state ->
        if (state is ConnectionState.Connected) {
            state.cameraID
        } else {
            DUMMY_CAMERA_ID
        }
    }

    companion object {
        const val DUMMY_CAMERA_ID = 0u
    }
}