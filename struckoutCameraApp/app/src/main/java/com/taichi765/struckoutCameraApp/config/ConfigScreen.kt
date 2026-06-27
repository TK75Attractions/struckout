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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel

@Composable
fun ConfigScreenRoute(
    onNavigateToCameraScreen: () -> Unit
) {
    val viewModel = hiltViewModel<ConfigViewModel>()
    val uiState by viewModel.uiState.collectAsState()

    ConfigScreen(
        uiState,
        onToggleNetworkFeature = viewModel::toggleNetworkFeature,
        onToggleRecordingMode = viewModel::toggleRecordingMode,
        onRetryConnection = viewModel::retryConnection,
        onDisableNetworkFeature = viewModel::disableNetworkFeature,
        onUpdateCameraLocation = { x, y, z ->
            viewModel.updateCameraLocation(x, y, z)
            onNavigateToCameraScreen()
        }
    )
}

@Composable
private fun ConfigScreen(
    uiState: ConfigUiState,
    onToggleRecordingMode: () -> Unit,
    onToggleNetworkFeature: () -> Unit,
    onDisableNetworkFeature: () -> Unit,
    onUpdateCameraLocation: (CharSequence, CharSequence, CharSequence) -> Unit,
    onRetryConnection: () -> Unit
) {
    val x = rememberTextFieldState((uiState.cameraLocation?.x ?: 0).toString())
    val y = rememberTextFieldState((uiState.cameraLocation?.y ?: 0).toString())
    val z = rememberTextFieldState((uiState.cameraLocation?.z ?: 0).toString())

    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Top,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        if (!uiState.networkFeatureEnabled || uiState.tcpIsConnected) {
            SwitchField("Recording Mode", uiState.recodingModeEnabled) {
                onToggleRecordingMode()
            }
            SwitchField("Network feature", uiState.networkFeatureEnabled) {
                onToggleNetworkFeature()
            }
        }
        if (uiState.tcpIsConnected) {
            CameraLocationView(
                x,
                y,
                z,
                uiState.warningState
            )
        }

        ConfirmButton {
            onUpdateCameraLocation(x.text, y.text, z.text)
        }
    }
    if (uiState.networkFeatureEnabled && !uiState.tcpIsConnected) {
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
    x: TextFieldState,
    y: TextFieldState,
    z: TextFieldState,
    warningState: WarningState,
) {
    Column(modifier = Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        Text("Camera Location", fontSize = 24.sp)
        PositionField("x", x, warningState.showX)
        PositionField("y", y, warningState.showY)
        PositionField("z", z, warningState.showZ)
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
private fun PositionField(text: String, textState: TextFieldState, showWarning: Boolean) {
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
    if (showWarning) {
        Text("error: cameraPositionには数字しか入力できません")
    }
}

@Composable
private fun ConfirmButton(onClick: () -> Unit) {
    Button(onClick = onClick) {
        Text("Done")
    }
}

@Preview(name = "Network enabled but not connected")
@Composable
private fun DisconnectedPreView() {
    ConfigScreen(
        uiState = ConfigUiState(
            tcpIsConnected = false,
            networkFeatureEnabled = true
        ),
        onToggleRecordingMode = {},
        onToggleNetworkFeature = {},
        onDisableNetworkFeature = {},
        onUpdateCameraLocation = { _, _, _ -> },
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
        onUpdateCameraLocation = { _, _, _ -> },
        onRetryConnection = {}
    )
}

@Preview(name = "Network connected")
@Composable
private fun ConnectedPreview() {
    ConfigScreen(
        uiState = ConfigUiState(
            tcpIsConnected = true
        ), onToggleRecordingMode = {},
        onToggleNetworkFeature = {},
        onDisableNetworkFeature = {},
        onUpdateCameraLocation = { _, _, _ -> },
        onRetryConnection = {})
}