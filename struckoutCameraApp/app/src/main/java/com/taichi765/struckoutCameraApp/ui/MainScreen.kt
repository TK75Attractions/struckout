package com.taichi765.struckoutCameraApp.ui

import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.lifecycle.viewmodel.compose.viewModel
import com.taichi765.struckoutCameraApp.MainViewModel


@Composable
fun MainScreen(viewModel: MainViewModel = viewModel()) {
    Text("Hello World")
}