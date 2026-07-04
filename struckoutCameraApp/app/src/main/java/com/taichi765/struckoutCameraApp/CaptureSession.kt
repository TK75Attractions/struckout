package com.taichi765.struckoutCameraApp

import dagger.hilt.android.scopes.ActivityRetainedScoped
import javax.inject.Inject
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

@ActivityRetainedScoped
@OptIn(ExperimentalUuidApi::class)
class CaptureSession @Inject constructor() {
    private var _sessionID = Uuid.generateV4()
    val sessionId = _sessionID

    fun reset() {
        _sessionID = Uuid.generateV4()
    }
}