package com.taichi765.struckoutCameraApp.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavController
import com.taichi765.struckoutCameraApp.ble.BleViewModel
import com.taichi765.struckoutCameraApp.ble.CameraLocation
import com.taichi765.struckoutCameraApp.transport.TcpTransportRepository

@Composable
fun CameraLocationScreen(
    tcpTransportRepository: TcpTransportRepository,
    navController: NavController
) {
    val viewModel = run {
        val factory = CameraLocationViewModel.Factory(tcpTransportRepository)
        viewModel<BleViewModel>(factory = factory)
    }
    val cameraLocation by viewModel.cameraLocation.collectAsState()

    CameraLocationView(
        cameraLocation = cameraLocation,
        onUpdateCameraLocation = {
            viewModel.updateCameraLocation(it)
            navController.navigate("camera")
        })
}

@Composable
private fun CameraLocationView(
    cameraLocation: CameraLocation?,
    onUpdateCameraLocation: (CameraLocation) -> Unit
) {
    val x = rememberTextFieldState((cameraLocation?.x ?: 0).toString())
    val y = rememberTextFieldState((cameraLocation?.y ?: 0).toString())
    val z = rememberTextFieldState((cameraLocation?.z ?: 0).toString())
    var showWarningTextX by remember { mutableStateOf(false) }

    var showWarningTextY by remember { mutableStateOf(false) }
    var showWarningTextZ by remember { mutableStateOf(false) }

    Column(modifier = Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        Text("Ble Info", fontSize = 24.sp)
        PositionField("x", x, showWarningTextX)
        PositionField("y", y, showWarningTextY)
        PositionField("z", z, showWarningTextZ)


        ConfirmButton {
            if (x.text.any { !it.isDigit() }) {
                showWarningTextX = true
                return@ConfirmButton
            }
            if (y.text.any { !it.isDigit() }) {
                showWarningTextY = true
                return@ConfirmButton
            }
            if (z.text.any { !it.isDigit() }) {
                showWarningTextZ = true
                return@ConfirmButton
            }
            val x = x.text.toString().toFloat()
            val y = y.text.toString().toFloat()
            val z = z.text.toString().toFloat()

            onUpdateCameraLocation(CameraLocation(x, y, z))
        }
    }
}


@Composable
private fun PositionField(text: String, textState: TextFieldState, showWarningState: Boolean) {
    TextField(
        textState,
        label = { Text(text) },
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        inputTransformation = InputTransformation {
            if (asCharSequence().any { !it.isDigit() }) {
                revertAllChanges()
            }
        }
    )
    if (showWarningState) {
        Text("error: cameraPositionには数字しか入力できません")
    }
}

@Composable
private fun ConfirmButton(onClick: () -> Unit) {
    Button(onClick = onClick) {
        Text("Update Position")
    }
}