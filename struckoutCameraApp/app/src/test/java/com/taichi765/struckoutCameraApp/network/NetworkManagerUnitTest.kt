package com.taichi765.struckoutCameraApp.network

import app.cash.turbine.test
import com.taichi765.struckoutCameraApp.FakeConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.DetectionOutputKind
import com.taichi765.struckoutCameraApp.network.types.InstanceState
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.impl.annotations.MockK
import io.mockk.junit5.MockKExtension
import io.mockk.verify
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith
import java.io.IOException
import java.net.SocketException

@OptIn(ExperimentalCoroutinesApi::class)
@ExtendWith(MockKExtension::class)
class NetworkManagerUnitTest {

    @Test
    fun `NetworkManager creates TcpSession and UdpConnection after network feature enabled`(
        @MockK tcpSession: TcpSession,
        @MockK tcpSessionFactory: TcpSession.Factory,
        @MockK udpConnection: UdpConnection,
        @MockK udpConnectionFactory: UdpConnection.Factory,
    ) = runTest {
        // Arrange
        coEvery { tcpSession.connect() } returns null
        every { tcpSessionFactory.create() } returns tcpSession
        every { tcpSession.state } returns MutableStateFlow(SessionState.DisConnected)
        coEvery { udpConnection.connect() } returns UdpConnectionError.UdpSocketError(
            SocketException("Stub!")
        )
        every { udpConnectionFactory.create() } returns udpConnection
        every { udpConnection.isConnected } returns MutableStateFlow(false)

        val configStoreRepository =
            FakeConfigStoreRepository(initialDetectionOutput = DetectionOutputKind.NONE)
        val networkManager = NetworkManager(
            configRepository = configStoreRepository,
            applicationScope = backgroundScope,
            tcpSessionFactory = tcpSessionFactory,
            udpConnectionFactory = udpConnectionFactory,
        )

        // Act 1
        networkManager.start()
        runCurrent()

        // Assert 1
        coVerify(exactly = 0) { tcpSession.connect() }
        coVerify(exactly = 0) { udpConnection.connect() }

        // Act 2
        configStoreRepository.setDetectionOutputKind(DetectionOutputKind.NETWORK)
        runCurrent()

        // Assert 2
        verify(exactly = 1) { tcpSessionFactory.create() }
        coVerify(exactly = 1) { tcpSession.connect() }
        verify(exactly = 1) { udpConnectionFactory.create() }
        coVerify(exactly = 1) { udpConnection.connect() }
    }

    @Test
    fun `tcpState updates after connection failed and succeeded`(
        @MockK tcpSession: TcpSession,
        @MockK tcpSessionFactory: TcpSession.Factory,
        @MockK udpConnection: UdpConnection,
        @MockK udpConnectionFactory: UdpConnection.Factory,
    ) = runTest {
        val sessionState = MutableStateFlow<SessionState>(SessionState.DisConnected)
        // Arrange
        coEvery { tcpSession.connect() } returns TcpSession.ConnectionError.TcpConnectionFailed(
            IOException("Stub!")
        )
        every { tcpSessionFactory.create() } returns tcpSession
        every { tcpSession.state } returns sessionState
        coEvery { udpConnection.connect() } returns UdpConnectionError.UdpSocketError(
            SocketException("Stub!")
        )
        every { udpConnection.isConnected } returns MutableStateFlow(false)
        every { udpConnectionFactory.create() } returns udpConnection

        val configStoreRepository = FakeConfigStoreRepository()

        val networkManager = NetworkManager(
            configRepository = configStoreRepository,
            applicationScope = backgroundScope,
            tcpSessionFactory = tcpSessionFactory,
            udpConnectionFactory = udpConnectionFactory,
        )

        backgroundScope.launch {
            // Assert
            networkManager.tcpState.test {
                assertEquals(InstanceState.NotCreated, awaitItem())

                awaitItem().let {
                    assert(it is InstanceState.Created && it.state.sessionState is SessionState.DisConnected && it.state.lastError == null)
                }

                awaitItem().let {
                    println("$it")
                    assert(
                        it is InstanceState.Created
                                && it.state.sessionState is SessionState.DisConnected
                                && it.state.lastError is TcpSession.ConnectionError.TcpConnectionFailed
                    )
                }
                awaitItem().let {
                    assert(
                        it is InstanceState.Created
                                && it.state.sessionState is SessionState.Connected
                    )
                }
            }
        }

        // Act 1
        networkManager.start()
        runCurrent()

        // Arrange 2
        coEvery { tcpSession.connect() } answers {
            sessionState.value = SessionState.Connected(99u)
            null
        }

        // Act 2
        networkManager.retryTcpConnection()
        runCurrent()
    }
}