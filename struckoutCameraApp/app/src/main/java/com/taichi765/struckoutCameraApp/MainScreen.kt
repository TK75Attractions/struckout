package com.taichi765.struckoutCameraApp

import androidx.compose.runtime.Composable
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.taichi765.struckoutCameraApp.camera.CameraScreen


@Composable
fun AppNavigation() {
    val navController = rememberNavController()


    NavHost(navController = navController, startDestination = "home") {
        composable("home") {
            CameraScreen()
        }
        composable("ble") {

        }
    }
}

