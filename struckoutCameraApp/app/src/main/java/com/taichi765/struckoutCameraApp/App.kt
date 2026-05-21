package com.taichi765.struckoutCameraApp

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CenterAlignedTopAppBar
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.taichi765.struckoutCameraApp.ble.BleManager
import com.taichi765.struckoutCameraApp.ble.BleScreen
import com.taichi765.struckoutCameraApp.camera.CameraScreen


@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun App() {
    val navController = rememberNavController()
    val bleManager = BleManager()

    Scaffold(
        topBar = {
            CenterAlignedTopAppBar(
                title = {
                    Text(
                        "Struckout Camera",
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                },
                actions = {
                    IconButton(onClick = {
                        navController.navigate("bleSettings")
                    }) {
                        Icon(
                            painter = painterResource(R.drawable.settings_24px),
                            contentDescription = "settings"
                        )
                    }
                })
        }
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = "home",
            modifier = Modifier.padding(innerPadding)
        ) {
            composable("home") {
                CameraScreen(bleManager)
            }
            composable("bleSettings") {
                BleScreen()
            }
        }
    }
}

