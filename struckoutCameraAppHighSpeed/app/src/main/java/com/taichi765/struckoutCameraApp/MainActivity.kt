package com.taichi765.struckoutCameraApp

import android.app.Application
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.taichi765.struckoutCameraApp.network.NetworkManager
import com.taichi765.struckoutCameraApp.remote.RemoteCommands
import com.taichi765.struckoutCameraApp.remote.RemoteControlReceiver
import dagger.hilt.android.AndroidEntryPoint
import dagger.hilt.android.HiltAndroidApp
import org.opencv.android.OpenCVLoader
import timber.log.Timber
import javax.inject.Inject

@HiltAndroidApp
class MainApplication : Application()

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    @Inject
    lateinit var networkManager: NetworkManager

    @Inject
    lateinit var remoteCommands: RemoteCommands

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        if (BuildConfig.ENABLE_NATS_LOG) {
            TODO("Sentryに置き換え")
        } else {
            Timber.plant(Timber.DebugTree())
        }

        OpenCVLoader.initLocal()

        networkManager.start()
        handleRemoteIntent(intent)

        setContent {
            AppTheme {
                App(remoteScreen = remoteCommands.screen, onRemoteScreenShown = remoteCommands::consumeScreen)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleRemoteIntent(intent)
    }

    /** PC から `am start ... --es screen camera` で開かれたときは、その画面にする */
    private fun handleRemoteIntent(intent: Intent?) {
        intent?.getStringExtra(RemoteControlReceiver.EXTRA_SCREEN)?.let(remoteCommands::requestScreen)
    }
}


