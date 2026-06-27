package com.taichi765.struckoutCameraApp.transport

import kotlinx.coroutines.flow.StateFlow

/**
 * Provides information about session state (e.g. camera id given by TCP server).
 */
interface SessionStateProvider {
    val state: StateFlow<SessionState>
}