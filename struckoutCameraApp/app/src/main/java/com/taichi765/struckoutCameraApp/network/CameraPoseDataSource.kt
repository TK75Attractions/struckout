package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.proto.CameraBallTracker
import kotlinx.coroutines.flow.Flow

interface CameraPoseDataSource {
    val cameraPose: Flow<CameraBallTracker.CameraPose>
}