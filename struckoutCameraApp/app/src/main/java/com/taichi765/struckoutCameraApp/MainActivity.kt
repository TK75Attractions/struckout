package com.taichi765.struckoutCameraApp

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import org.opencv.android.OpenCVLoader
import timber.log.Timber

class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        if (BuildConfig.ENABLE_NATS_LOG) {
            Timber.plant(NatsLoggingTree())
        } else {
            Timber.plant(Timber.DebugTree())
        }

        OpenCVLoader.initLocal()

        setContent {
            AppTheme {
                App()
            }
        }
    }
}


