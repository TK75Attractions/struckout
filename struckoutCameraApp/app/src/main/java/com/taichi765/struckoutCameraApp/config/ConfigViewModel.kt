package com.taichi765.struckoutCameraApp.config

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.proto.Struckout
import com.taichi765.struckoutCameraApp.proto.cameraLocation
import com.taichi765.struckoutCameraApp.transport.SessionState
import com.taichi765.struckoutCameraApp.transport.TcpSession
import com.taichi765.struckoutCameraApp.transport.UdpDetectionRepository
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
    private val tcpSession: TcpSession,
    udpDetectionRepository: UdpDetectionRepository,
    private val configRepository: ConfigStoreRepository
) : ViewModel() {
    /**
     * TODO: [TcpSession]に持たせる
     */
    private val _cameraLocation = MutableStateFlow<Struckout.CameraLocation?>(null)
    private val _warningState = MutableStateFlow(WarningState())

    private val sessionState =
        combine(
            tcpSession.state,
            udpDetectionRepository.isBound
        ) { tcpState, udpIsBound ->
            Pair(tcpState, udpIsBound)
        }

    /**
     * invariant: `isConnected` should be always `false` if `networkFeature` is disabled.
     *
     * TODO: isConnectedとnetworkFeatureEnabledをsealed interfaceにする
     */
    val uiState = combine(
        configRepository.recordingModeEnabled,
        configRepository.networkFeatureEnabled,
        sessionState,
        _cameraLocation,
        _warningState
    ) { recodingModeEnabled, networkFeatureEnabled, sessionState, cameraLocation, warningState ->
        ConfigUiState(
            recodingModeEnabled,
            networkFeatureEnabled,
            tcpIsConnected = sessionState.first is SessionState.Connected,
            udpIsBound = sessionState.second,
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

    fun connect() {
        viewModelScope.launch {
            tcpSession.connect()
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
            configRepository.toggleRecodingMode()
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


