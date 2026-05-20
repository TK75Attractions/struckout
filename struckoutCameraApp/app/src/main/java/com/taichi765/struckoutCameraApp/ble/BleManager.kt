package com.taichi765.struckoutCameraApp.ble

import android.util.Log
import com.juul.kable.Characteristic
import com.juul.kable.Filter
import com.juul.kable.Peripheral
import com.juul.kable.Scanner
import com.juul.kable.characteristicOf
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

@OptIn(ExperimentalUuidApi::class)
private val SERVICE_UUID = Uuid.parse("d575b50d-cfd8-4747-b6cd-1aa0ffce1108")

@OptIn(ExperimentalUuidApi::class)
private val CAMERA_POSITION_CHARACTERISTIC_UUID = Uuid.parse("a4b3a793-ff34-47a0-847b-32b54cba0d6f")

@OptIn(ExperimentalUuidApi::class)
private val FRAME_CHARACTERISTIC_UUID = Uuid.parse("bda5d9c9-0c9a-4e45-b20b-1fb937e71a7d")

@OptIn(ExperimentalUuidApi::class)
class BleManager() {
    private val scanner = Scanner {
        filters { match { name = Filter.Name.Exact("Struckout") } }
    }

    private var peripheral: Peripheral? = null
    private var cameraPositionCharacteristic: Characteristic? = null
    private var frameCharacteristic: Characteristic? = null

    suspend fun connect() {
        Log.i(TAG, "scanning advertisements...")
        val advertisement = scanner.advertisements.first()
        Log.i(TAG, "found advertisement: $advertisement")
        val autoConnect = MutableStateFlow(false)
        peripheral = Peripheral(advertisement) {
            autoConnectIf { autoConnect.value }
        }

        cameraPositionCharacteristic =
            characteristicOf(SERVICE_UUID, CAMERA_POSITION_CHARACTERISTIC_UUID)
        frameCharacteristic = characteristicOf(SERVICE_UUID, FRAME_CHARACTERISTIC_UUID)
    }


    suspend fun sendFrame(data: FrameData) {
        peripheral?.write(frameCharacteristic!!, data.toByteArray())
    }

    suspend fun updateCameraLocation(loc: CameraLocation) {
        peripheral?.write(cameraPositionCharacteristic!!, loc.toByteArray())

    }

    companion object {
        const val TAG = "BleManager"
    }
}

/// Represents data sent to `frame` characteristic.
data class FrameData(val frameID: FrameID, val x: Float, val y: Float, val z: Float)


fun FrameData.toByteArray(): ByteArray {
    return ByteBuffer
        .allocate(16)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putInt(frameID.id.toInt())
        .putFloat(x)
        .putFloat(y)
        .putFloat(z)
        .array()
}

data class CameraLocation(val x: Float, val y: Float, val z: Float)

fun CameraLocation.toByteArray(): ByteArray {
    return ByteBuffer
        .allocate(12)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putFloat(x)
        .putFloat(y)
        .putFloat(z)
        .array()
}

data class FrameID(val id: UInt)