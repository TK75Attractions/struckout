package com.taichi765.struckoutCameraApp.config

import com.taichi765.struckoutCameraApp.CaptureSession
import com.taichi765.struckoutCameraApp.FakeConfigStoreRepository
import com.taichi765.struckoutCameraApp.MainDispatcherExtension
import com.taichi765.struckoutCameraApp.camera.DetectionArea
import com.taichi765.struckoutCameraApp.network.NetworkManager
import com.taichi765.struckoutCameraApp.network.types.ConnectionState
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@OptIn(ExperimentalCoroutinesApi::class)
@ExtendWith(MainDispatcherExtension::class)
class ConfigViewModelUnitTest {
    @Test
    fun `blank input is rejected with a field error and no settings are saved`() = runTest {
        val (viewModel, repository) = createViewModel()
        runCurrent()

        val accepted = viewModel.applyChanges(
            viewModel.uiState.value.copy(
                locationX = " ",
                detectionOutputKind = DetectionOutputKind.LOCAL,
                highSpeedStreamSize = HighSpeedStreamSize.VGA,
                recodingModeEnabled = true,
            )
        )
        runCurrent()

        assertFalse(accepted)
        assertEquals("位置 Xを入力してください。", viewModel.validationErrors.value[ConfigField.LOCATION_X])
        assertEquals(0.0, repository.focalLengthRatio.value)
        assertEquals(DetectionArea(), repository.detectionArea.value)
        assertEquals(0.0, repository.cameraPose.value.x)
        assertEquals(DetectionOutputKind.NETWORK, repository.detectionOutputKind.value)
        assertEquals(HighSpeedStreamSize.HD720, repository.highSpeedStreamSize.value)
        assertFalse(repository.recordingModeEnabled.value)
    }

    @Test
    fun `non numeric input is rejected`() = runTest {
        val (viewModel) = createViewModel()
        runCurrent()

        assertFalse(viewModel.applyChanges(viewModel.uiState.value.copy(rotationYDegrees = "abc")))
        assertEquals("回転 Y（pitch）は数値で入力してください。", viewModel.validationErrors.value[ConfigField.ROTATION_Y_DEGREES])
    }

    @Test
    fun `NaN and infinity are rejected`() = runTest {
        val (viewModel) = createViewModel()
        runCurrent()

        assertFalse(viewModel.applyChanges(viewModel.uiState.value.copy(locationZ = "NaN")))
        assertTrue(viewModel.validationErrors.value[ConfigField.LOCATION_Z]!!.contains("有限"))

        assertFalse(viewModel.applyChanges(viewModel.uiState.value.copy(rotationXDegrees = "Infinity")))
        assertTrue(viewModel.validationErrors.value[ConfigField.ROTATION_X_DEGREES]!!.contains("有限"))
    }

    @Test
    fun `valid negative position and decimal values are saved`() = runTest {
        val (viewModel, repository) = createViewModel()
        runCurrent()
        val input = viewModel.uiState.value.copy(
            locationX = "-1.25",
            locationY = "2.5",
            focalLengthRatio = "0.75",
            ignoredLeftPercent = "1.5",
            ignoredRightPercent = "2.25",
        )

        assertTrue(viewModel.applyChanges(input))
        runCurrent()

        assertEquals(-1.25, repository.cameraPose.value.x)
        assertEquals(2.5, repository.cameraPose.value.y)
        assertEquals(0.75, repository.focalLengthRatio.value)
        assertEquals(DetectionArea(1.5, 0.0, 2.25, 0.0), repository.detectionArea.value)
        assertTrue(viewModel.validationErrors.value.isEmpty())
    }

    private fun createViewModel(): Pair<ConfigViewModel, FakeConfigStoreRepository> {
        val repository = FakeConfigStoreRepository()
        val networkManager = mockk<NetworkManager>()
        every { networkManager.state } returns MutableStateFlow(ConnectionState.NetworkOutputDisabled)
        return ConfigViewModel(networkManager, repository, CaptureSession()) to repository
    }
}
