package com.taichi765.struckoutCameraApp.recording

import app.cash.turbine.test
import com.taichi765.struckoutCameraApp.network.LocalDetectionUploader
import com.taichi765.struckoutCameraApp.network.LocalDetectionUploader.ConnectionError
import com.taichi765.struckoutCameraApp.proto.detectionsPacket
import com.taichi765.struckoutCameraApp.recording.LocalDataViewModel.ConnectionStatus
import com.taichi765.struckoutCameraApp.recording.LocalDataViewModel.UploadStatus
import io.mockk.coEvery
import io.mockk.every
import io.mockk.impl.annotations.MockK
import io.mockk.junit5.MockKExtension
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith
import java.io.IOException
import kotlin.time.Duration.Companion.milliseconds

@OptIn(ExperimentalCoroutinesApi::class)
@ExtendWith(MockKExtension::class)
class LocalDataViewModelUnitTest {

    @Test
    fun `connectionStatus is updated after connection succeeds`(
        @MockK localDetectionRepository: LocalDetectionRepository,
        @MockK localDetectionUploader: LocalDetectionUploader,
    ) = runTest {
        // Arrange
        val isConnected = MutableStateFlow(false)
        coEvery { localDetectionUploader.connect() } coAnswers {
            delay(100.milliseconds)
            isConnected.value = true
            null
        }
        every { localDetectionUploader.isConnected } returns isConnected.asStateFlow()
        every { localDetectionRepository.rowCount } returns flowOf(100)

        // Act
        val viewModel = LocalDataViewModel(
            localDetectionRepository = localDetectionRepository,
            localDetectionUploader = localDetectionUploader
        )

        // Assert
        backgroundScope.launch {
            viewModel.connectionStatus.test {
                assert(awaitItem() is ConnectionStatus.NoAttempts)

                assert(awaitItem() is ConnectionStatus.Connected)
            }
        }

        runCurrent()
    }

    @Test
    fun `connectionStatus is updated after retrying connection succeeds`(
        @MockK localDetectionRepository: LocalDetectionRepository,
        @MockK localDetectionUploader: LocalDetectionUploader,
    ) = runTest {
        // Arrange 1
        val isConnected = MutableStateFlow(false)
        coEvery { localDetectionUploader.connect() } coAnswers {
            delay(100.milliseconds)
            ConnectionError.TcpConnection(
                IOException("Stub!")
            )
        }
        every { localDetectionUploader.isConnected } returns isConnected.asStateFlow()
        every { localDetectionRepository.rowCount } returns flowOf(100)

        // Act 1
        val viewModel = LocalDataViewModel(
            localDetectionRepository = localDetectionRepository,
            localDetectionUploader = localDetectionUploader
        )

        // Assert
        backgroundScope.launch {
            viewModel.connectionStatus.test {
                assert(awaitItem() is ConnectionStatus.NoAttempts)

                awaitItem().let {
                    assert(it is ConnectionStatus.Error && it.error is ConnectionError.TcpConnection)
                }

                assert(awaitItem() is ConnectionStatus.Connected)
            }
        }
        runCurrent()

        // Arrange 2
        coEvery { localDetectionUploader.connect() } coAnswers {
            delay(100.milliseconds)
            isConnected.value = true
            null
        }
        // Act 2
        viewModel.connect()
        runCurrent()
    }

    @Test
    fun `LocalDataViewModel retries connection after existing connection is aborted`(
        @MockK localDetectionRepository: LocalDetectionRepository,
        @MockK localDetectionUploader: LocalDetectionUploader,
    ) = runTest {
        // Arrange
        val isConnected = MutableStateFlow(false)
        coEvery { localDetectionUploader.connect() } coAnswers {
            delay(100.milliseconds)
            isConnected.value = true
            null
        }
        every { localDetectionUploader.isConnected } returns isConnected.asStateFlow()
        every { localDetectionRepository.rowCount } returns flowOf(100)

        val viewModel = LocalDataViewModel(
            localDetectionRepository = localDetectionRepository,
            localDetectionUploader = localDetectionUploader
        )

        // Assert
        backgroundScope.launch {
            viewModel.connectionStatus.test {
                assert(awaitItem() is ConnectionStatus.NoAttempts)
                assert(awaitItem() is ConnectionStatus.Connected)
                assert(awaitItem() is ConnectionStatus.NoAttempts)
                assert(awaitItem() is ConnectionStatus.Connected)
            }
        }
        runCurrent()

        // Act
        isConnected.value = false
        runCurrent()
    }

    @Test
    fun `uploadStatus is updated while uploading`(
        @MockK localDetectionRepository: LocalDetectionRepository,
        @MockK localDetectionUploader: LocalDetectionUploader,
    ) = runTest {
        // Arrange
        val isConnected = MutableStateFlow(false)
        coEvery { localDetectionUploader.connect() } answers {
            isConnected.value = true
            null
        }
        every { localDetectionUploader.isConnected } returns isConnected.asStateFlow()
        coEvery { localDetectionUploader.upload(any()) } coAnswers {
            delay(200.milliseconds) // 実際の挙動に近づける
            null
        }
        every { localDetectionRepository.rowCount } returns flowOf(100)
        coEvery { localDetectionRepository.loadAll() } returns listOf(
            FrameEntity(
                timestamp = 1_000_000_000,
                data = detectionsPacket { })
        )


        val viewModel = LocalDataViewModel(
            localDetectionRepository = localDetectionRepository,
            localDetectionUploader = localDetectionUploader
        )

        backgroundScope.launch {
            viewModel.uploadStatus.test {
                assert(awaitItem() is UploadStatus.NotStarted)

                assert(awaitItem() is UploadStatus.InProgress)

                assert(awaitItem() is UploadStatus.Succeed)
            }
        }
        runCurrent()

        viewModel.uploadLocalDetections()
        runCurrent()
    }
}