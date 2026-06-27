package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.flow.Flow

interface CameraLocationDataSource {
    val cameraLocation: Flow<Struckout.CameraLocation>
}