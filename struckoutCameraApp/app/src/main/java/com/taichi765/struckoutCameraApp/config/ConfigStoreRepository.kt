package com.taichi765.struckoutCameraApp.config

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.doublePreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.taichi765.struckoutCameraApp.di.ApplicationScope
import com.taichi765.struckoutCameraApp.proto.Struckout
import com.taichi765.struckoutCameraApp.proto.cameraLocation
import com.taichi765.struckoutCameraApp.transport.CameraLocationDataSource
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "config")


class ConfigStoreRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    @ApplicationScope private val scope: CoroutineScope
) : CameraLocationDataSource {
    val recordingModeEnabled = context.dataStore.data.map { preferences ->
        preferences[ENABLE_RECORDING_MODE] ?: ENABLE_RECORDING_MODE_DEFAULT
    }.stateIn(
        scope = scope,
        started = SharingStarted.WhileSubscribed(5000),
        initialValue = ENABLE_RECORDING_MODE_DEFAULT
    )// アプリケーション全体で共有される状態なのでRepository内でstateInしても不自然ではない

    val networkFeatureEnabled = context.dataStore.data.map { preferences ->
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

    suspend fun toggleRecodingMode() {
        context.dataStore.updateData {
            it.toMutablePreferences().also { preferences ->
                preferences[ENABLE_RECORDING_MODE] =
                    !(preferences[ENABLE_RECORDING_MODE] ?: ENABLE_RECORDING_MODE_DEFAULT)
            }
        }
    }

    suspend fun toggleNetworkFeature() {
        context.dataStore.updateData {
            it.toMutablePreferences().also { preferences ->
                preferences[ENABLE_NETWORK_FEATURE] =
                    !(preferences[ENABLE_NETWORK_FEATURE] ?: ENABLE_NETWORK_FEATURE_DEFAULT)
            }
        }
    }

    suspend fun disableNetworkFeature() {
        context.dataStore.updateData {
            it.toMutablePreferences().also { preferences ->
                preferences[ENABLE_NETWORK_FEATURE] = false
            }
        }
    }

    suspend fun updateCameraLocation(cameraLocation: Struckout.CameraLocation) {
        context.dataStore.updateData {
            it.toMutablePreferences().also { preferences ->
                preferences[CAMERA_LOCATION_Y] = cameraLocation.x
                preferences[CAMERA_LOCATION_Y] = cameraLocation.y
                preferences[CAMERA_LOCATION_Z] = cameraLocation.z
            }
        }
    }

    companion object {
        const val ENABLE_RECORDING_MODE_DEFAULT = false
        const val ENABLE_NETWORK_FEATURE_DEFAULT = true

        private val ENABLE_RECORDING_MODE = booleanPreferencesKey("enable_recording_mode")
        private val ENABLE_NETWORK_FEATURE = booleanPreferencesKey("enable_network")
        private val CAMERA_LOCATION_X = doublePreferencesKey("camera_location_x")
        private val CAMERA_LOCATION_Y = doublePreferencesKey("camera_location_y")
        private val CAMERA_LOCATION_Z = doublePreferencesKey("camera_location_z")
    }
}
