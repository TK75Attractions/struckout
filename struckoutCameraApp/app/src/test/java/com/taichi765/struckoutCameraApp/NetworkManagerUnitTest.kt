package com.taichi765.struckoutCameraApp

import com.taichi765.struckoutCameraApp.network.NetworkManager
import com.taichi765.struckoutCameraApp.network.TcpSession
import com.taichi765.struckoutCameraApp.network.types.SessionState
import io.mockk.coJustRun
import io.mockk.coVerify
import io.mockk.every
import io.mockk.impl.annotations.MockK
import io.mockk.junit5.MockKExtension
import io.mockk.verify
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(MockKExtension::class)
class NetworkManagerUnitTest {

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test
    fun `NetworkManager creates TcpSession and UdpConnection after network feature enabled`(
        @MockK tcpSession: TcpSession,
        @MockK tcpSessionFactory: TcpSession.Factory
    ) = runTest {
        // Arrange
        coJustRun { tcpSession.connect() }
        every { tcpSessionFactory.create() } returns tcpSession
        every { tcpSession.state } returns MutableStateFlow(SessionState.DisConnected)

        val configStoreRepository =
            FakeConfigStoreRepository(initialNetworkFeatureEnabled = false)
        val networkManager = NetworkManager(
            configRepository = configStoreRepository,
            applicationScope = backgroundScope,
            tcpSessionFactory = tcpSessionFactory
        )

        // Act1
        networkManager.start()
        runCurrent()

        // Assert1
        coVerify(exactly = 0) { tcpSession.connect() }

        // Act2
        configStoreRepository.toggleNetworkFeature()
        runCurrent()

        // Assert2
        verify(exactly = 1) { tcpSessionFactory.create() }
        coVerify(exactly = 1) { tcpSession.connect() }
    }
}