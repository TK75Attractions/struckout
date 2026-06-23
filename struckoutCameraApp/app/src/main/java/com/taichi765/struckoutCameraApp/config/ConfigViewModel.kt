package com.taichi765.struckoutCameraApp.config

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.camera.types.CameraLocation
import com.taichi765.struckoutCameraApp.proto.TcpClientPacketKt
import com.taichi765.struckoutCameraApp.proto.cameraLocation
import com.taichi765.struckoutCameraApp.proto.tcpClientPacket
import com.taichi765.struckoutCameraApp.transport.ConnectionState
import com.taichi765.struckoutCameraApp.transport.TcpTransportRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import timber.log.Timber

class ConfigViewModel(
    private val tcpRepository: TcpTransportRepository,
    private val configRepository: ConfigStoreRepository
) : ViewModel() {
    /**
     * TODO: [TcpTransportRepository]に持たせる
     */
    private val _cameraLocation = MutableStateFlow<CameraLocation?>(null)

    private val _connState = tcpRepository.state.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Eagerly,
        initialValue = ConnectionState.Disconnected
    )

    /**
     * invariant: `isConnected` should be always `false` if `networkFeature` is disabled.
     *
     * TODO: isConnectedとnetworkFeatureEnabledをsealed interfaceにする
     */
    val uiState = combine(
        configRepository.recordingModeEnabled,
        configRepository.networkFeatureEnabled,
        _cameraLocation,
        _connState.map {
            when (it) {
                is ConnectionState.Connected -> true
                is ConnectionState.Disconnected -> false
            }
        }
    ) { recodingModeEnabled, networkFeatureEnabled, cameraLocation, isConnected ->
        ConfigUiState(
            recodingModeEnabled, networkFeatureEnabled, isConnected, cameraLocation,
        )
    }.stateIn(
        viewModelScope,
        started = SharingStarted.Eagerly,
        initialValue = ConfigUiState()
    )

    fun updateCameraLocation(value: CameraLocation) {
        val curState = _connState.value
        if (curState !is ConnectionState.Connected) {
            Timber.tag(TAG).w("cannot update camera location: TCP is disconnected")
            return
        }
        Timber.tag(TAG).i("updating camera location")
        _cameraLocation.value = value
        val packet = tcpClientPacket {
            cameraLoc = TcpClientPacketKt.updateCameraLocation {
                this.cameraLocation = cameraLocation {
                    x = value.x
                    y = value.y
                    z = value.z
                }
                cameraId = curState.cameraID.toInt()
            }
        }
        viewModelScope.launch {
            tcpRepository.sendPacket(packet)
        }
    }

    fun connect() {
        viewModelScope.launch {
            tcpRepository.connect()
        }
    }

    fun close() {
        viewModelScope.launch {
            tcpRepository.close()
        }
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

    class Factory(
        val tcpRepository: TcpTransportRepository,
        val configRepository: ConfigStoreRepository
    ) : ViewModelProvider.Factory {
        @Suppress("UNCHECKED_CAST")
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return ConfigViewModel(tcpRepository, configRepository) as T
        }
    }

    companion object {
        const val TAG = "ConfigViewModel"
    }
}


