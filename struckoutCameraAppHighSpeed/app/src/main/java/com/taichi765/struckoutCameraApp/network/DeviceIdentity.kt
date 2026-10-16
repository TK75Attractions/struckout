package com.taichi765.struckoutCameraApp.network

import android.annotation.SuppressLint
import android.content.Context
import android.provider.Settings
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * ball_tracker にこの端末を見分けてもらうための識別子。
 *
 * [Settings.Secure.ANDROID_ID] は、端末とアプリの署名の組ごとに決まり、アプリの設定を消しても、入れ直しても変わらない
 * (端末を初期化すると変わる。別の PC で署名したアプリを入れたときも変わる)。adb のシリアルはアプリからは読めない。
 * PC は `scripts/camera-remote.ps1 config` でこの値を聞き、`scripts/cameras.json` に書いておく。
 */
@Singleton
class DeviceIdentity @Inject constructor(@ApplicationContext context: Context) {
    @SuppressLint("HardwareIds")
    val deviceId: String = Settings.Secure.getString(context.contentResolver, Settings.Secure.ANDROID_ID) ?: ""
}
