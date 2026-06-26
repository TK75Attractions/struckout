package com.taichi765.struckoutCameraApp

import android.app.Application
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.taichi765.struckoutCameraApp.transport.ConnectionManager
import dagger.hilt.android.AndroidEntryPoint
import dagger.hilt.android.HiltAndroidApp
import org.opencv.android.OpenCVLoader
import timber.log.Timber
import javax.inject.Inject

@HiltAndroidApp
class MainApplication : Application() {}

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    @Inject
    lateinit var connectionManager: ConnectionManager

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        if (BuildConfig.ENABLE_NATS_LOG) {
            Timber.plant(NatsLoggingTree())
        } else {
            Timber.plant(Timber.DebugTree())
        }

        OpenCVLoader.initLocal()

        connectionManager.start()

        setContent {
            AppTheme {
                App()
            }
        }
    }
}


