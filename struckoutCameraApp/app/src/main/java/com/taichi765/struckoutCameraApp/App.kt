package com.taichi765.struckoutCameraApp

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager.PERMISSION_GRANTED
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.CenterAlignedTopAppBar
import androidx.compose.material3.ExperimentalMaterial3Api
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.taichi765.struckoutCameraApp.camera.CameraScreen
import com.taichi765.struckoutCameraApp.settings.CameraLocationScreen
import com.taichi765.struckoutCameraApp.transport.TcpTransport
import com.taichi765.struckoutCameraApp.transport.UdpTransport

val REQUIRED_PERMISSIONS = arrayOf(
    Manifest.permission.CAMERA,
    Manifest.permission.ACCESS_FINE_LOCATION,
    Manifest.permission.ACCESS_COARSE_LOCATION
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun App() {
    val navController = rememberNavController()
    val context = LocalContext.current
    var permissionGranted by remember { mutableStateOf(checkCurrentPermission(context)) }

    val tcpRepository = TcpTransport()
    val udpRepository = UdpTransport()

    Scaffold(
        topBar = {
            CenterAlignedTopAppBar(
                title = {
                    Text(
                        "Struckout Camera",
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            )
        }
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = if (permissionGranted) "settings" else "permissionRequired",
            modifier = Modifier.padding(innerPadding)
        ) {
            composable("camera") {
                CameraScreen(udpRepository, tcpRepository, navController)
            }
            composable("settings") {
                CameraLocationScreen(tcpTransportRepository = tcpRepository, navController)
            }
            composable("permissionRequired") {
                PermissionRequestScreen { permissionGranted = true }
            }
        }
    }
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

