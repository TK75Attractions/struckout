package com.taichi765.struckoutCameraApp.recording

import android.content.ContentValues
import android.content.Context
import android.provider.MediaStore
import com.taichi765.struckoutCameraApp.recording.VideoEncoder.Companion.TAG
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.flow
import timber.log.Timber
import javax.inject.Inject
import kotlin.time.Clock
import kotlin.time.Duration.Companion.minutes

class RecordingManager @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    init {
        flow {
            while (true) {
                emit(Unit)
                delay(10.minutes)
            }
        }.collect {
            val now = Clock.System.now().toString()
        }
    }

    private fun flashToMediaStore() {
        Timber.tag(TAG).d("flashing recorded video to MediaStore")
        val resolver = context.contentResolver
        val values = ContentValues().apply {
            put(MediaStore.Video.Media.DISPLAY_NAME, "my_video")
            put(MediaStore.Video.Media.MIME_TYPE, "video/mp4")
        }
        val uri =
            resolver.insert(MediaStore.Video.Media.EXTERNAL_CONTENT_URI, values)
                ?: throw IllegalStateException("failed to insert to MediaStore")
        resolver.openOutputStream(uri).use {
            TODO()
        }
    }
}