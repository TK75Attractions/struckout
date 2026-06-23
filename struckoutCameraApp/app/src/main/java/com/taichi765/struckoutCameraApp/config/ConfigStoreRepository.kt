package com.taichi765.struckoutCameraApp.config

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "config")

/**
 * Lifetime of this repository is as same as [android.app.Activity],
 * so you should use `applicationScope`.
 */
class ConfigStoreRepository(private val context: Context, private val scope: CoroutineScope) {
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

    companion object {
        const val ENABLE_RECORDING_MODE_DEFAULT = false
        const val ENABLE_NETWORK_FEATURE_DEFAULT = true

        private val ENABLE_RECORDING_MODE = booleanPreferencesKey("enable_recording_mode")
        private val ENABLE_NETWORK_FEATURE = booleanPreferencesKey("enable_network")
    }
}
