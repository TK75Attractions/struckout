package com.taichi765.struckoutCameraApp.config

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.map

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "config")

class ConfigStoreRepository(private val context: Context) {
    fun enableRecordingModeFlow() = context.dataStore.data.map { preferences ->
        preferences[ENABLE_RECORDING_MODE] ?: ENABLE_RECORDING_MODE_DEFAULT
    }

    fun enableNetworkFlow() = context.dataStore.data.map { preferences ->
        preferences[ENABLE_NETWORK] ?: ENABLE_NETWORK_DEFAULT
    }

    suspend fun toggleRecodingMode() = context.dataStore.updateData {
        it.toMutablePreferences().also { preferences ->
            preferences[ENABLE_RECORDING_MODE] =
                !(preferences[ENABLE_RECORDING_MODE] ?: ENABLE_RECORDING_MODE_DEFAULT)
        }
    }

    suspend fun toggleNetwork() = context.dataStore.updateData {
        it.toMutablePreferences().also { preferences ->
            preferences[ENABLE_NETWORK] = !(preferences[ENABLE_NETWORK] ?: ENABLE_NETWORK_DEFAULT)
        }
    }

    companion object {
        private const val ENABLE_RECORDING_MODE_DEFAULT = false
        private const val ENABLE_NETWORK_DEFAULT = true

        private val ENABLE_RECORDING_MODE = booleanPreferencesKey("enable_recording_mode")
        private val ENABLE_NETWORK = booleanPreferencesKey("enable_network")
    }
}
