package com.taichi765.struckoutCameraApp.settings

import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.taichi765.struckoutCameraApp.camera.types.CameraLocation
import com.taichi765.struckoutCameraApp.transport.ConnectionState
import com.taichi765.struckoutCameraApp.transport.TcpTransportRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import timber.log.Timber

class CameraLocationViewModel(private val tcpRepository: TcpTransportRepository) : ViewModel() {
    private val _cameraLocation = MutableStateFlow<CameraLocation?>(null)
    val cameraLocation = _cameraLocation.asStateFlow()

    val connState = tcpRepository.state.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Eagerly,
        initialValue = ConnectionState.Disconnected
    )

    val isConnected = connState.map {
        when (it) {
            is ConnectionState.Connected -> true
            is ConnectionState.Disconnected -> false
        }
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Eagerly,
        initialValue = false
    )

    suspend fun updateCameraLocation(value: CameraLocation) {
        val curState = connState.value
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
        tcpRepository.sendPacket(packet)
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

    class Factory(val tcpRepository: TcpTransportRepository) : ViewModelProvider.Factory {
        @Suppress("UNCHECKED_CAST")
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return CameraLocationViewModel(tcpRepository) as T
        }
    }

    companion object {
        const val TAG = "CameraLocationViewModel"
    }
}

