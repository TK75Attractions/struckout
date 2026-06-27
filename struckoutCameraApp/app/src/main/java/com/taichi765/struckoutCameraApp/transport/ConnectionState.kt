package com.taichi765.struckoutCameraApp.transport

sealed interface ConnectionState {
    data class NetworkFeatureEnabled(
        val tcpInstanceState: InstanceState<TcpSession.SessionState>,
        val udpInstanceState: InstanceState<Boolean>
    ) : ConnectionState

    object NetworkFeatureDisabled : ConnectionState
}

sealed interface InstanceState<out T> {
    data class Created<T>(val state: T) : InstanceState<T>
    object NotCreated : InstanceState<Nothing>
}