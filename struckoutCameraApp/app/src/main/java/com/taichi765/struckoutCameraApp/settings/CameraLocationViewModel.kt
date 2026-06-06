package com.taichi765.struckoutCameraApp.settings

import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import com.taichi765.struckoutCameraApp.CameraLocation
import com.taichi765.struckoutCameraApp.transport.TcpTransportRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import struckout.cameraLocation
import struckout.tcpClientPacket

class CameraLocationViewModel(private val tcpRepository: TcpTransportRepository) : ViewModel() {
    private val _cameraLocation = MutableStateFlow<CameraLocation?>(null)
    val cameraLocation = _cameraLocation.asStateFlow()

    suspend fun updateCameraLocation(value: CameraLocation) {
        Log.i(TAG, "updating camera location")
        _cameraLocation.value = value
        val packet = tcpClientPacket {
            this.cameraLocation = cameraLocation {
                x = value.x
                y = value.y
                z = value.z
            }
        }
        tcpRepository.sendPacket(packet)
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

