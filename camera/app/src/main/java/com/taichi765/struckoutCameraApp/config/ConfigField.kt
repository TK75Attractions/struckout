package com.taichi765.struckoutCameraApp.config

enum class ConfigField(val label: String) {
    LOCATION_X("位置 X"),
    LOCATION_Y("位置 Y"),
    LOCATION_Z("位置 Z"),
    ROTATION_X_DEGREES("回転 X（roll）"),
    ROTATION_Y_DEGREES("回転 Y（pitch）"),
    ROTATION_Z_DEGREES("回転 Z（yaw）"),
    FOCAL_LENGTH_RATIO("焦点距離比"),
    IGNORED_LEFT_PERCENT("検出無視幅 左"),
    IGNORED_TOP_PERCENT("検出無視幅 上"),
    IGNORED_RIGHT_PERCENT("検出無視幅 右"),
    IGNORED_BOTTOM_PERCENT("検出無視幅 下"),
}
