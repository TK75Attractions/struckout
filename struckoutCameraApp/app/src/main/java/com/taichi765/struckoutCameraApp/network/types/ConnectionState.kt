package com.taichi765.struckoutCameraApp.network.types

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.DetectionOutputKind
import com.taichi765.struckoutCameraApp.network.SessionState
import com.taichi765.struckoutCameraApp.network.TcpSession

sealed interface ConnectionState {
    /**
     * [DetectionOutputKind] is set to [DetectionOutputKind.NETWORK] in [ConfigStoreRepository].
     */
    data class NetworkOutputEnabled(
        val tcpInstanceState: InstanceState<TcpState>,
        val dataConnInstanceState: InstanceState<Boolean>,
    ) : ConnectionState

    object NetworkOutputDisabled : ConnectionState
}

sealed interface InstanceState<out T> {
    data class Created<T>(val state: T) : InstanceState<T>
    object NotCreated : InstanceState<Nothing>
}

data class TcpState(val sessionState: SessionState, val lastError: TcpSession.ConnectionError?)

fun ConnectionState.tcpIsConnected(): Boolean {
    return this is ConnectionState.NetworkOutputEnabled
            && this.tcpInstanceState is InstanceState.Created
            && this.tcpInstanceState.state.sessionState is SessionState.Connected
}

fun ConnectionState.udpIsConnected(): Boolean {
    return this is ConnectionState.NetworkOutputEnabled
            && this.dataConnInstanceState is InstanceState.Created
            && this.dataConnInstanceState.state
}