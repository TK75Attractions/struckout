package com.taichi765.struckoutCameraApp.ble

import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onCompletion
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.onEmpty
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeout
import no.nordicsemi.kotlin.ble.client.android.CentralManager
import no.nordicsemi.kotlin.ble.client.android.Peripheral
import no.nordicsemi.kotlin.ble.client.distinctByPeripheral
import kotlin.time.Duration.Companion.milliseconds
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

private const val SERVICE_UUID = "d575b50d-cfd8-4747-b6cd-1aa0ffce1108";
private const val CAMERA_POSITION_CHARACTERISTIC_UUID = "a4b3a793-ff34-47a0-847b-32b54cba0d6f";
private const val FRAME_CHARACTERISTIC_UUID = "bda5d9c9-0c9a-4e45-b20b-1fb937e71a7d";

class BleManager(
    private val centralManager: CentralManager,
    private val scope: CoroutineScope
) {
    private var peripheral: Peripheral? = null

    @OptIn(ExperimentalUuidApi::class)
    private fun findPeripheral() {
        Log.i(TAG, "finding peripheral")
        centralManager.scan(1250.milliseconds) {
            ServiceUuid(
                Uuid.parse(SERVICE_UUID)
            )
        }.distinctByPeripheral().map { it.peripheral }.onEach {
            peripheral = it
        }.launchIn(scope)
    }

    private fun connect() {
        if (peripheral == null) {
            return
        }
        scope.launch {
            withTimeout(10000) {
                centralManager.connect(
                    peripheral = peripheral!!,// TODO: 汚い
                    options = CentralManager.ConnectionOptions.AutoConnect()
                )
                Log.i(TAG, "connected to ${peripheral!!.name}")
            }
        }

        peripheral!!.phy
            .onEach {
                Log.i(TAG, "PHY changed to: $it")
            }
            .onEmpty {
                Log.w(TAG, "PHY didn't change")
            }
            .onCompletion {
                Log.d(TAG, "PHY collection completed")
            }
            .launchIn(scope)
    }

    companion object {
        const val TAG = "BleManager"
    }
}


