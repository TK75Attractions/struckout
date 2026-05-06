package com.taichi765.struckoutCameraApp

import android.app.Service
import android.content.ComponentName
import android.content.Intent
import android.content.ServiceConnection
import android.os.Binder
import android.os.IBinder

class BLEManager {
    private var bluetoothService: BluetoothService? = null

    private val servceConnection = object : ServiceConnection {
        override fun onServiceConnected(componentName: ComponentName?, service: IBinder?) {
            bluetoothService = (service as BluetoothService.LocalBinder).getService()
            bluetoothService?.let { bluetooth ->
                println("do something here")
            }
        }

        override fun onServiceDisconnected(componentName: ComponentName?) {
            bluetoothService = null
        }
    }
}

class BluetoothService : Service() {
    val binder = LocalBinder()

    override fun onBind(intent: Intent?): IBinder {
        return binder
    }

    inner class LocalBinder : Binder() {
        fun getService(): BluetoothService {
            return this@BluetoothService
        }
    }
}

