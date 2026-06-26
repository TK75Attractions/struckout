package com.taichi765.struckoutCameraApp.transport

import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.flow.Flow

interface CameraLocationDataSource {
    val cameraLocation: Flow<Struckout.CameraLocation>
}