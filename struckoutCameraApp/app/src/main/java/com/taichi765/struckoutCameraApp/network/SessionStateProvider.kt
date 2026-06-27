package com.taichi765.struckoutCameraApp.network

import com.taichi765.struckoutCameraApp.network.types.SessionState
import kotlinx.coroutines.flow.StateFlow

/**
 * Provides information about session state (e.g. camera id given by TCP server).
 */
interface SessionStateProvider {
    val state: StateFlow<SessionState>
}