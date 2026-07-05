package com.taichi765.struckoutCameraApp.config

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.doublePreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository.Companion.ENABLE_NETWORK_FEATURE_DEFAULT
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository.Companion.ENABLE_RECORDING_MODE_DEFAULT
import com.taichi765.struckoutCameraApp.di.ApplicationScope
import com.taichi765.struckoutCameraApp.proto.Struckout
import com.taichi765.struckoutCameraApp.proto.cameraLocation
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject
import javax.inject.Singleton

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "config")

@Singleton
class ConfigStoreRepositoryImpl @Inject constructor(
    @ApplicationContext private val context: Context,
    @ApplicationScope private val scope: CoroutineScope
) : ConfigStoreRepository {
    override val recordingModeEnabled = context.dataStore.data.map { preferences ->
        preferences[ENABLE_RECORDING_MODE] ?: ENABLE_RECORDING_MODE_DEFAULT
    }.stateIn(
        scope = scope,
        started = SharingStarted.WhileSubscribed(5000),
        initialValue = ENABLE_RECORDING_MODE_DEFAULT
    )// アプリケーション全体で共有される状態なのでRepository内でstateInしても不自然ではない

    override val networkFeatureEnabled = context.dataStore.data.map { preferences ->
        preferences[ENABLE_NETWORK_FEATURE] ?: ENABLE_NETWORK_FEATURE_DEFAULT
    }.stateIn(
        scope = scope,
        started = SharingStarted.WhileSubscribed(5000),
        initialValue = ENABLE_NETWORK_FEATURE_DEFAULT
    )// アプリケーション全体で共有される状態なのでRepository内でstateInしても不自然ではない

    override val cameraLocation = context.dataStore.data.map { preferences ->
        cameraLocation {
            x = preferences[CAMERA_LOCATION_X] ?: 0.0
            y = preferences[CAMERA_LOCATION_Y] ?: 0.0
            z = preferences[CAMERA_LOCATION_Z] ?: 0.0
        }
    }.stateIn(
        scope = scope,
        started = SharingStarted.Eagerly,
        initialValue = cameraLocation {
            x = 0.0
            y = 0.0
            z = 0.0
        }
    )

    override val detectionOutputKind: StateFlow<DetectionOutputKind> =
        context.dataStore.data.map { preferences ->
            preferences[DETECTION_OUTPUT_KIND]?.let {
                DetectionOutputKind.valueOf(it)
            } ?: DetectionOutputKind.LOCAL
        }.stateIn(
            scope = scope,
            started = SharingStarted.Eagerly,
            initialValue = DetectionOutputKind.LOCAL
        )

    override suspend fun setDetectionOutputKind(kind: DetectionOutputKind) {
        context.dataStore.updateData {
            it.toMutablePreferences().also { preferences ->
                preferences[DETECTION_OUTPUT_KIND] = kind.name
            }
        }
    }

    override suspend fun toggleRecordingMode() {
        context.dataStore.updateData {
            it.toMutablePreferences().also { preferences ->
                preferences[ENABLE_RECORDING_MODE] =
                    !(preferences[ENABLE_RECORDING_MODE] ?: ENABLE_RECORDING_MODE_DEFAULT)
            }
        }
    }

    override suspend fun updateCameraLocation(location: Struckout.CameraLocation) {
        context.dataStore.updateData {
            it.toMutablePreferences().also { preferences ->
                preferences[CAMERA_LOCATION_Y] = location.x
                preferences[CAMERA_LOCATION_Y] = location.y
                preferences[CAMERA_LOCATION_Z] = location.z
            }
        }
    }

    companion object {
        const val TAG = "ConfigStoreRepository"

        private val ENABLE_RECORDING_MODE = booleanPreferencesKey("enable_recording_mode")
        private val ENABLE_NETWORK_FEATURE = booleanPreferencesKey("enable_network")
        private val CAMERA_LOCATION_X = doublePreferencesKey("camera_location_x")
        private val CAMERA_LOCATION_Y = doublePreferencesKey("camera_location_y")
        private val CAMERA_LOCATION_Z = doublePreferencesKey("camera_location_z")
        private val DETECTION_OUTPUT_KIND = stringPreferencesKey("detection_output_kind")
    }
}
