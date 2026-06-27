package com.taichi765.struckoutCameraApp.config

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.network.NetworkManager
import com.taichi765.struckoutCameraApp.network.TcpSession
import com.taichi765.struckoutCameraApp.network.types.ConnectionState
import com.taichi765.struckoutCameraApp.network.types.tcpIsConnected
import com.taichi765.struckoutCameraApp.network.types.udpIsConnected
import com.taichi765.struckoutCameraApp.proto.Struckout
import com.taichi765.struckoutCameraApp.proto.cameraLocation
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import timber.log.Timber
import javax.inject.Inject

@HiltViewModel
class ConfigViewModel @Inject constructor(
    private val networkManager: NetworkManager,
    private val configRepository: ConfigStoreRepository
) : ViewModel() {
    /**
     * TODO: [TcpSession]に持たせる
     */
    private val _cameraLocation = MutableStateFlow<Struckout.CameraLocation?>(null)
    private val _warningState = MutableStateFlow(WarningState())

    /**
     * invariant: `isConnected` should be always `false` if `networkFeature` is disabled.
     *
     * TODO: isConnectedとnetworkFeatureEnabledをsealed interfaceにする
     */
    val uiState = combine(
        configRepository.recordingModeEnabled,
        networkManager.state,
        _cameraLocation,
        _warningState
    ) { recodingModeEnabled, connectionState, cameraLocation, warningState ->
        ConfigUiState(
            recodingModeEnabled,
            networkFeatureEnabled = connectionState is ConnectionState.NetworkFeatureEnabled,
            tcpIsConnected = connectionState.tcpIsConnected(),
            udpIsConnected = connectionState.udpIsConnected(),
            cameraLocation = cameraLocation,
            warningState = warningState
        )
    }.stateIn(
        viewModelScope,
        started = SharingStarted.Eagerly,
        initialValue = ConfigUiState()
    )

    fun updateCameraLocation(x: CharSequence, y: CharSequence, z: CharSequence) {
        if (validateCameraLocationState(x, y, z)) return

        Timber.tag(TAG).i("updating camera location")
        val x = x.toString().toDouble()
        val y = y.toString().toDouble()
        val z = z.toString().toDouble()

        val cameraLocation = cameraLocation {
            this.x = x
            this.y = y
            this.z = z
        }

        viewModelScope.launch {
            configRepository.updateCameraLocation(cameraLocation)
        }
    }

    fun retryConnection() {
        viewModelScope.launch {
            networkManager.retryConnection()
        }
    }

    /**
     * @return `true` if valid, `false` if invalid.
     */
    private fun validateCameraLocationState(
        x: CharSequence,
        y: CharSequence,
        z: CharSequence
    ): Boolean {
        // reset state
        _warningState.value = WarningState()

        if (x.any { !it.isDigit() }) {
            _warningState.value = _warningState.value.copy(showX = true)
        }
        if (y.any { !it.isDigit() }) {
            _warningState.value = _warningState.value.copy(showY = true)
        }
        if (z.any { !it.isDigit() }) {
            _warningState.value = _warningState.value.copy(showZ = true)
        }
        return _warningState.value.isAllOk()
    }

    fun toggleRecordingMode() {
        viewModelScope.launch {
            configRepository.toggleRecordingMode()
        }
    }

    fun toggleNetworkFeature() {
        viewModelScope.launch {
            configRepository.toggleNetworkFeature()
        }
    }

    fun disableNetworkFeature() {
        check(configRepository.networkFeatureEnabled.value) {
            "disableNetworkFeature should not be called when it's already disabled"
        }
        viewModelScope.launch {
            configRepository.disableNetworkFeature()
        }
    }


    companion object {
        const val TAG = "ConfigViewModel"
    }
}


