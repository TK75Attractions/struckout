package com.taichi765.struckoutCameraApp

import com.taichi765.struckoutCameraApp.ble.BleManager
import no.nordicsemi.kotlin.ble.client.mock.PeripheralSpec
import org.junit.Test

class BleUnitTest {
    private val hrm = PeripheralSpec

    @Test
    fun bleManager_canFindPeripheral() {
        val centralManager
        val bleManager = BleManager()
    }
}