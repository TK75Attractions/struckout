package com.taichi765.struckoutCameraApp.network.types

sealed interface SessionState {
    data class Connected(val cameraID: UInt) : SessionState

    object DisConnected : SessionState
}