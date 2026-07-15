package com.taichi765.struckoutCameraApp.config

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel

@Composable
fun ConfigScreenRoute(
    onNavigateToCameraScreen: () -> Unit,
    onPopBackNavStack: () -> Unit
) {
    val viewModel = hiltViewModel<ConfigViewModel>()
    val savedState by viewModel.uiState.collectAsState()

    var editingState by remember { mutableStateOf(savedState.copy()) }

    val radioOptions = listOf("Network", "Local", "None")
    val (selectedOption, setSelectedOption) = remember { mutableStateOf(radioOptions[0]) }
    val x = rememberTextFieldState(savedState.locationX.toString())
    val y = rememberTextFieldState(savedState.locationY.toString())
    val z = rememberTextFieldState(savedState.locationZ.toString())

    var showDiscardDialog by remember { mutableStateOf(false) }


    ConfigScreen(
        recordingModeEnabled = editingState.recodingModeEnabled,
        connectionFailed = savedState.detectionOutputKind == DetectionOutputKind.NETWORK && !savedState.tcpIsConnected,
        x = x,
        y = y,
        z = z,
        onToggleRecordingMode = {
            editingState =
                editingState.copy(recodingModeEnabled = !editingState.recodingModeEnabled)
        },
        onSetDetectionOutput = { kind ->
            editingState = editingState.copy(detectionOutputKind = kind)
        },
        showDiscardDialog = showDiscardDialog,
        radioOptions = radioOptions,
        selectedOption = selectedOption,
        onOptionSelected = { option ->
            setSelectedOption(option)
            val kind = textToDetectionOutputKind(option)
                ?: throw IllegalStateException("Unknown radio option: $option")
            editingState = editingState.copy(detectionOutputKind = kind)
        },
        onRetryConnection = viewModel::retryConnection,
        onApplyChanges = {
            editingState = editingState
                .copy(
                    locationX = x.text,
                    locationY = y.text,
                    locationZ = z.text
                )
            viewModel.applyChanges(editingState)
            onNavigateToCameraScreen()
        },
        onDiscardChanges = {
            showDiscardDialog = false
            onPopBackNavStack()
        }
    )

    // TODO: BackHandlerだと戻る動作しかフックできないのでNavControllerのラッパーを作る
    BackHandler(savedState != editingState) {
        showDiscardDialog = true
    }
}

private fun textToDetectionOutputKind(text: String): DetectionOutputKind? {
    return when (text) {
        "Network" -> DetectionOutputKind.NETWORK
        "Local" -> DetectionOutputKind.LOCAL
        "None" -> DetectionOutputKind.NONE
        else -> null
    }
}

@Composable
private fun ConfigScreen(
    recordingModeEnabled: Boolean,
    connectionFailed: Boolean,
    showDiscardDialog: Boolean,
    radioOptions: List<String>,
    selectedOption: String,
    x: TextFieldState,
    y: TextFieldState,
    z: TextFieldState,
    onToggleRecordingMode: () -> Unit,
    onRetryConnection: () -> Unit,
    onSetDetectionOutput: (DetectionOutputKind) -> Unit,
    onOptionSelected: (String) -> Unit,
    onApplyChanges: () -> Unit,
    onDiscardChanges: () -> Unit
) {
    if (connectionFailed) {
        FallbackView(onTryConnect = {
            onRetryConnection()
        }, onSetDetectionOutput = onSetDetectionOutput)
    } else {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            FeatureConfigCard(
                recordingModeEnabled = recordingModeEnabled,
                radioOptions = radioOptions,
                selectedOption = selectedOption,
                onOptionSelected = onOptionSelected,
                onToggleRecordingMode = onToggleRecordingMode,
            )

            CameraLocationInput(
                x,
                y,
                z,
            )

            ConfirmButton(onClick = onApplyChanges)
        }
    }

    if (showDiscardDialog) {
        DiscardDialog(onDiscardChanges = onDiscardChanges, onApplyChanges = onApplyChanges)
    }
}

// TODO: 別のところに移す
@Composable
private fun SessionCard(onResetSession: () -> Unit) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp),
        shape = RoundedCornerShape(16.dp)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Button(onClick = onResetSession) {
                Text("Reset session")
            }
        }
    }
}

@Composable
private fun FeatureConfigCard(
    recordingModeEnabled: Boolean,
    radioOptions: List<String>,
    selectedOption: String,
    onToggleRecordingMode: () -> Unit,
    onOptionSelected: (String) -> Unit
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.Top,
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            SwitchField("Recording Mode", recordingModeEnabled) {
                onToggleRecordingMode()
            }
            DetectionOutputKindSelection(
                radioOptions = radioOptions,
                selectedOption = selectedOption,
                onOptionSelected = onOptionSelected
            )
        }
    }
}

@Composable
private fun DetectionOutputKindSelection(
    radioOptions: List<String>,
    selectedOption: String,
    onOptionSelected: (String) -> Unit
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp)
    ) {
        Column(modifier = Modifier.selectableGroup()) {
            Text(
                text = "検知結果の出力先",
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.padding(bottom = 16.dp)
            )

            radioOptions.forEach { text ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(56.dp)
                        .selectable(
                            selected = (text == selectedOption),
                            onClick = { onOptionSelected(text) },
                            role = Role.RadioButton
                        )
                        .padding(horizontal = 16.dp)
                ) {
                    RadioButton(selected = (text == selectedOption), onClick = null)
                    Text(
                        text = text,
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.padding(start = 16.dp)
                    )
                }
            }
        }
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
private fun CameraLocationInput(
    x: TextFieldState,
    y: TextFieldState,
    z: TextFieldState,
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text("Camera Location", fontSize = 24.sp)
            PositionField("x", x)
            PositionField("y", y)
            PositionField("z", z)
        }
    }
}


@Composable
private fun PositionField(text: String, textState: TextFieldState) {
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
}

@Composable
private fun DiscardDialog(onDiscardChanges: () -> Unit, onApplyChanges: () -> Unit) {
    Dialog(onDismissRequest = onDiscardChanges) {
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .height(375.dp)
                .padding(16.dp),
            shape = RoundedCornerShape(16.dp)
        ) {
            Column(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Text("設定の変更を保存しますか？")
                Row(modifier = Modifier.fillMaxWidth()) {
                    TextButton(onClick = onDiscardChanges) {
                        Text("破棄")
                    }
                    TextButton(onClick = onApplyChanges) {
                        Text("保存")
                    }
                }
            }
        }
    }
}

@Composable
private fun FallbackView(
    onTryConnect: () -> Unit,
    onSetDetectionOutput: (DetectionOutputKind) -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxSize(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp, alignment = Alignment.CenterVertically)
    ) {
        Text("Detection output is set to 'Network', but TCP is not connected.")

        Button(onClick = onTryConnect) {
            Text("Retry connection")
        }
        Button(onClick = { onSetDetectionOutput(DetectionOutputKind.NONE) }) {
            Text("Change detection output to 'None'")
        }
        Button(onClick = { onSetDetectionOutput(DetectionOutputKind.LOCAL) }) {
            Text("Change detection output to 'Local'")
        }
    }
}

@Composable
private fun ConfirmButton(onClick: () -> Unit) {
    Button(onClick = onClick) {
        Text("Done")
    }
}

/**
 * [ConfigScreen] filled with dummy parameters. This should be only used from the function marked as [Preview].
 */
@Composable
private fun DummyConfigScreen(
    recordingModeEnabled: Boolean = true,
    connectionFailed: Boolean = false,
    showDiscardDialog: Boolean = false,
    radioOptions: List<String> = listOf("Network", "Local", "None"),
    selectedOption: String = "Network",
    x: TextFieldState = TextFieldState(),
    y: TextFieldState = TextFieldState(),
    z: TextFieldState = TextFieldState()
) {
    ConfigScreen(
        recordingModeEnabled = recordingModeEnabled,
        connectionFailed = connectionFailed,
        showDiscardDialog = showDiscardDialog,
        radioOptions = radioOptions,
        selectedOption = selectedOption,
        x = x,
        y = y,
        z = z,
        onToggleRecordingMode = {},
        onRetryConnection = {},
        onSetDetectionOutput = {},
        onOptionSelected = {},
        onApplyChanges = {},
        onDiscardChanges = {}
    )
}

@Preview(name = "Network enabled but not connected")
@Composable
private fun DisconnectedPreView() {
    DummyConfigScreen(
        connectionFailed = true,
        selectedOption = "Network",
    )
}

@Preview(name = "Not using network output")
@Composable
private fun NetworkDisabledPreview() {
    DummyConfigScreen(
        selectedOption = "Local",
    )
}

@Preview(name = "Network connected")
@Composable
private fun ConnectedPreview() {
    DummyConfigScreen(
        selectedOption = "Network",
        connectionFailed = false
    )
}