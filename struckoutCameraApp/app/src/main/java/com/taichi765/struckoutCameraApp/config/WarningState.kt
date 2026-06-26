package com.taichi765.struckoutCameraApp.config

/**
 * `true`のとき警告を表示する
 */
data class WarningState(
    val showX: Boolean = false,
    val showY: Boolean = false,
    val showZ: Boolean = false
)

fun WarningState.isAllOk(): Boolean {
    if (showX) return false
    if (showY) return false
    if (showZ) return false
    return true
}