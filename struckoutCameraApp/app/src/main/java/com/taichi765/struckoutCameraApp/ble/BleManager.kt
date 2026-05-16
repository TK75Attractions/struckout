package com.taichi765.struckoutCameraApp.ble

import android.util.Log
import com.juul.kable.Filter
import com.juul.kable.Peripheral
import com.juul.kable.Scanner
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

@OptIn(ExperimentalUuidApi::class)
private val SERVICE_UUID = Uuid.parse("d575b50d-cfd8-4747-b6cd-1aa0ffce1108")

@OptIn(ExperimentalUuidApi::class)
private val CAMERA_POSITION_CHARACTERISTIC_UUID = Uuid.parse("a4b3a793-ff34-47a0-847b-32b54cba0d6f")

@OptIn(ExperimentalUuidApi::class)
private val FRAME_CHARACTERISTIC_UUID = Uuid.parse("bda5d9c9-0c9a-4e45-b20b-1fb937e71a7d")

class BleManager() {
    private val scanner = Scanner {
        filters { match { name = Filter.Name.Exact("Struckout") } }
    }


    suspend fun connect() {
        Log.i(TAG, "scanning advertisements...")
        val advertisement = scanner.advertisements.first()
        Log.i(TAG, "found advertisement: $advertisement")
        val autoConnect = MutableStateFlow(false)
        val peripheral = Peripheral(advertisement) {
            autoConnectIf { autoConnect.value }
        }
    }

    private suspend fun scanPeripheral() {

    }

    suspend fun sendFrame(frameId: FrameID) {
        TODO()
    }

    companion object {
        const val TAG = "BleManager"
    }
}

data class FrameID(val id: UInt)

