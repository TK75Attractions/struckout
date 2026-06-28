package com.taichi765.struckoutCameraApp.network.types

import com.taichi765.struckoutCameraApp.network.SessionState

sealed interface ConnectionState {
    data class NetworkFeatureEnabled(
        val tcpInstanceState: InstanceState<SessionState>,
        val udpInstanceState: InstanceState<Boolean>,
        val synchronizerInstanceState: InstanceState<Boolean>
    ) : ConnectionState

    object NetworkFeatureDisabled : ConnectionState
}

sealed interface InstanceState<out T> {
    data class Created<T>(val state: T) : InstanceState<T>
    object NotCreated : InstanceState<Nothing>
}

fun ConnectionState.tcpIsConnected(): Boolean {
    return this is ConnectionState.NetworkFeatureEnabled
            && this.tcpInstanceState is InstanceState.Created
            && this.tcpInstanceState.state is SessionState.Connected
}

fun ConnectionState.udpIsConnected(): Boolean {
    return this is ConnectionState.NetworkFeatureEnabled
            && this.udpInstanceState is InstanceState.Created
            && this.udpInstanceState.state
}

fun ConnectionState.synchronizerIsConnected(): Boolean {
    return this is ConnectionState.NetworkFeatureEnabled
            && this.synchronizerInstanceState is InstanceState.Created
            && this.synchronizerInstanceState.state
}