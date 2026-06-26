package com.taichi765.struckoutCameraApp.transport

sealed interface SessionState {
    data class Connected(val cameraID: UInt) : SessionState

    object DisConnected : SessionState
}