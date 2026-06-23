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
import androidx.compose.material3.Button
import androidx.compose.material3.CenterAlignedTopAppBar
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.taichi765.struckoutCameraApp.camera.CameraController
import com.taichi765.struckoutCameraApp.camera.CameraScreenRoute
import com.taichi765.struckoutCameraApp.camera.CameraViewModel
import com.taichi765.struckoutCameraApp.config.ConfigScreenRoute
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.ConfigViewModel
import com.taichi765.struckoutCameraApp.transport.TcpTransport
import com.taichi765.struckoutCameraApp.transport.UdpDetectionRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

val REQUIRED_PERMISSIONS = arrayOf(
    Manifest.permission.CAMERA,
    Manifest.permission.ACCESS_FINE_LOCATION,
    Manifest.permission.ACCESS_COARSE_LOCATION,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun App() {
    val applicationScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    val navController = rememberNavController()
    val context = LocalContext.current
    var permissionGranted by remember { mutableStateOf(checkCurrentPermission(context)) }

    val tcpRepository = TcpTransport()
    val udpRepository = UdpDetectionRepository()
    val configRepository = ConfigStoreRepository(context, applicationScope)

    val configViewModel = run {
        val factory = ConfigViewModel.Factory(tcpRepository, configRepository)
        viewModel<ConfigViewModel>(factory = factory)
    }
    val cameraViewModel = run {
        val cameraController = CameraController(context)
        val factory = CameraViewModel.Factory(
            udpRepository,
            tcpRepository,
            cameraController,
            configRepository
        )
        viewModel<CameraViewModel>(factory = factory)
    }

    Scaffold(
        topBar = { TopBar(navController) },
        modifier = Modifier.safeContentPadding()
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = if (permissionGranted) "config" else "permissionRequired",
            modifier = Modifier.padding(innerPadding)
        ) {
            composable("camera") {
                CameraScreenRoute(cameraViewModel)
            }
            composable("config") {
                ConfigScreenRoute(
                    configViewModel,
                    navController
                )
            }
            composable("permissionRequired") {
                PermissionRequestScreen { permissionGranted = true }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TopBar(navController: NavController) {
    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentRoute = navBackStackEntry?.destination?.route

    CenterAlignedTopAppBar(
        title = {
            Text(
                "Struckout Camera",
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
        },
        navigationIcon = {
            if (currentRoute != "config") {
                IconButton(onClick = {
                    navController.navigate("config")
                }) {
                    Icon(
                        painter = painterResource(R.drawable.settings_24px),
                        contentDescription = "settings"
                    )
                }
            }
        }
    )
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

