package com.taichi765.struckoutCameraApp

import android.content.Context
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.taichi765.struckoutCameraApp.camera.CameraView

@Composable
fun MainScreen(viewModel: MainViewModel = viewModel()) {
    Text("Hello World")
}

@Composable
fun AppNavigation(context: Context) {
    val navController = rememberNavController()


    NavHost(navController = navController, startDestination = "home") {
        composable("home") {
            CameraView(context)
        }
    }
}

