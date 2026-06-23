package com.taichi765.struckoutCameraApp.config

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.Button
import androidx.compose.material3.Switch
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.navigation.NavController
import com.taichi765.struckoutCameraApp.camera.types.CameraLocation

@Composable
fun ConfigScreenRoute(
    viewModel: ConfigViewModel,
    navController: NavController
) {
    val uiState by viewModel.uiState.collectAsState()

    ConfigScreen(
        uiState,
        onToggleNetworkFeature = viewModel::toggleNetworkFeature,
        onToggleRecordingMode = viewModel::toggleRecordingMode,
        onRetryConnection = viewModel::connect,
        onDisableNetworkFeature = viewModel::disableNetworkFeature,
        onUpdateCameraLocation = {
            viewModel.updateCameraLocation(it)
            navController.navigate("camera")
        }
    )
}

@Composable
private fun ConfigScreen(
    uiState: ConfigUiState,
    onToggleRecordingMode: () -> Unit,
    onToggleNetworkFeature: () -> Unit,
    onDisableNetworkFeature: () -> Unit,
    onUpdateCameraLocation: (CameraLocation) -> Unit,
    onRetryConnection: () -> Unit
) {
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Top
    ) {
        if (!uiState.networkFeatureEnabled || uiState.isConnected) {
            SwitchField("Recording Mode", uiState.recodingModeEnabled) {
                onToggleRecordingMode()
            }
            SwitchField("Network feature", uiState.networkFeatureEnabled) {
                onToggleNetworkFeature()
            }
        }
        if (uiState.isConnected) {
            CameraLocationView(
                cameraLocation = uiState.cameraLocation,
                onUpdateCameraLocation = {
                    onUpdateCameraLocation(uiState.cameraLocation!!)
                }
            )
        }
    }
    if (uiState.networkFeatureEnabled && !uiState.isConnected) {
        FallbackView(onTryConnect = {
            onRetryConnection()
        }, onDisableNetworkFeature = {
            onDisableNetworkFeature()
        })
    }
}


@Composable
private fun SwitchField(text: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) {
        Text(text)
        Switch(checked, onCheckedChange)
    }
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
        Text("Camera Location", fontSize = 24.sp)
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
            val x = x.text.toString().toDouble()
            val y = y.text.toString().toDouble()
            val z = z.text.toString().toDouble()

            onUpdateCameraLocation(CameraLocation(x, y, z))
        }
    }
}

@Composable
private fun FallbackView(onTryConnect: () -> Unit, onDisableNetworkFeature: () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxSize(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp, alignment = Alignment.CenterVertically)
    ) {
        Text("Network feature is enabled but TCP is not connected.")

        Button(onClick = onTryConnect) {
            Text("Retry connection")
        }
        Button(onClick = onDisableNetworkFeature) {
            Text("Disable network feature")
        }
    }
}


@Composable
private fun PositionField(text: String, textState: TextFieldState, showWarningState: Boolean) {
    TextField(
        textState,
        label = { Text(text) },
        keyboardOptions = KeyboardOptions(
            keyboardType = KeyboardType.Number,
            imeAction = ImeAction.Next
        ),
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

@Preview(name = "Network enabled but not connected")
@Composable
private fun DisconnectedPreView() {
    ConfigScreen(
        uiState = ConfigUiState(
            isConnected = false,
            networkFeatureEnabled = true
        ),
        onToggleRecordingMode = {},
        onToggleNetworkFeature = {},
        onDisableNetworkFeature = {},
        onUpdateCameraLocation = {},
        onRetryConnection = {},
    )
}

@Preview(name = "Network feature disabled")
@Composable
private fun NetworkDisabledPreview() {
    ConfigScreen(
        uiState = ConfigUiState(
            networkFeatureEnabled = false
        ),
        onToggleRecordingMode = {},
        onToggleNetworkFeature = {},
        onDisableNetworkFeature = {},
        onUpdateCameraLocation = {},
        onRetryConnection = {}
    )
}