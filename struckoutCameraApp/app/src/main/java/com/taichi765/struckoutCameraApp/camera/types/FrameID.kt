package com.taichi765.struckoutCameraApp.camera.types

import androidx.annotation.CheckResult

data class FrameID(val id: UInt)

@CheckResult
fun FrameID.increment(): FrameID {
    return FrameID(id + 1u)
}

@CheckResult
fun FrameID.toLong(): Long {
    return id.toLong()
}