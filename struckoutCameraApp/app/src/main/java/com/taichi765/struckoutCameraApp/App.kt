package com.taichi765.struckoutCameraApp

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager.PERMISSION_GRANTED
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeContentPadding
import androidx.compose.material3.BottomAppBar
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.navigation.NavController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.taichi765.struckoutCameraApp.camera.CameraScreenRoute
import com.taichi765.struckoutCameraApp.config.ConfigScreenRoute
import com.taichi765.struckoutCameraApp.recording.RecordingDataScreenRoute

val REQUIRED_PERMISSIONS = arrayOf(
    Manifest.permission.CAMERA,
    Manifest.permission.ACCESS_FINE_LOCATION,
    Manifest.permission.ACCESS_COARSE_LOCATION,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun App() {
    val navController = rememberNavController()
    val context = LocalContext.current
    var permissionGranted by remember { mutableStateOf(checkCurrentPermission(context)) }

    Scaffold(
        bottomBar = {
            BottomBar(navController = navController)
        },
        modifier = Modifier.safeContentPadding()
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = if (permissionGranted) "config" else "permissionRequired",
            modifier = Modifier.padding(innerPadding)
        ) {
            composable("data") {
                RecordingDataScreenRoute()
            }
            composable("camera") {
                CameraScreenRoute()
            }
            composable("config") {
                ConfigScreenRoute(
                    onNavigateToCameraScreen = {
                        navController.navigate("camera")
                    }
                )
            }
            composable("permissionRequired") {
                PermissionRequestScreen { permissionGranted = true }
            }
        }
    }
}

@Composable
private fun BottomBar(navController: NavController) {
    BottomAppBar(actions = {
        IconButton(onClick = {
            navController.navigate("data")
        }) {
            Icon(Icons.database, contentDescription = "data")
        }
        IconButton(onClick = {
            navController.navigate("camera")
        }) {
            Icon(imageVector = Icons.photo_camera, contentDescription = "camera")
        }
        IconButton(onClick = {
            navController.navigate("config")
        }) {
            Icon(
                painter = painterResource(R.drawable.settings_24px),
                contentDescription = "config"
            )
        }
    })
}

@Composable
private fun PermissionRequestScreen(onAllPermissionGranted: () -> Unit) {
    val launcher =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { result ->
            if (result.all { it.value }) {
                onAllPermissionGranted()
            }
        }
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Button(onClick = {
            launcher.launch(
                REQUIRED_PERMISSIONS
            )
        }) {
            Text("Request Permission")
        }
    }
}

private fun checkCurrentPermission(context: Context): Boolean {
    return REQUIRED_PERMISSIONS.all { context.checkSelfPermission(it) == PERMISSION_GRANTED }
}

