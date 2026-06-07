package com.taichi765.struckoutCameraApp.camera.types

data class FrameID(val id: UInt)

fun FrameID.increment(): FrameID {
    return FrameID(id + 1u)
}