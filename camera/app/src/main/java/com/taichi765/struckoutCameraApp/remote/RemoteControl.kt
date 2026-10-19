package com.taichi765.struckoutCameraApp.remote

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.annotation.CallSuper
import com.taichi765.struckoutCameraApp.camera.DetectionArea
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.DetectionOutputKind
import com.taichi765.struckoutCameraApp.di.ApplicationScope
import com.taichi765.struckoutCameraApp.network.DeviceIdentity
import com.taichi765.struckoutCameraApp.network.NetworkManager
import com.taichi765.struckoutCameraApp.network.types.tcpIsConnected
import com.taichi765.struckoutCameraApp.proto.cameraPose
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import timber.log.Timber
import javax.inject.Inject
import javax.inject.Singleton

/**
 * PC から adb で受けた命令のうち、画面 (ViewModel) に渡すもの。
 */
@Singleton
class RemoteCommands @Inject constructor() {
    private val _saveFrame = MutableSharedFlow<Unit>(extraBufferCapacity = 1)

    /** 「Save frame」を押したのと同じ。カメラ画面が開いているときだけ効く */
    val saveFrame: SharedFlow<Unit> = _saveFrame.asSharedFlow()

    private val _screen = MutableStateFlow<String?>(null)

    /** 開いてほしい画面 ("camera" など)。画面を切り替えたら [consumeScreen] で消す */
    val screen: StateFlow<String?> = _screen.asStateFlow()

    /** カメラ画面が開いていて、カメラが動いている (状態の問い合わせに答えるため) */
    @Volatile
    var cameraScreenActive = false

    fun requestSaveFrame(): Boolean = _saveFrame.tryEmit(Unit)

    fun requestScreen(screen: String) {
        _screen.value = screen
    }

    fun consumeScreen() {
        _screen.value = null
    }
}

/**
 * 設置したあとで画面に触れずに済むよう、PC から adb で操作するための受け口 (`scripts/camera_remote.py`)。
 *
 * ```
 * adb shell am broadcast -a com.taichi765.struckoutCameraApp.remote.SET_CONFIG -p <アプリ ID> \
 *     --es output network --es focal_ratio 0.928 --es pose 0.11,0.12,1.23,137.365,-1.534,-31.474
 * adb shell am broadcast -a com.taichi765.struckoutCameraApp.remote.SAVE_FRAME -p <アプリ ID>
 * adb shell am broadcast -a com.taichi765.struckoutCameraApp.remote.STATUS -p <アプリ ID>
 * ```
 *
 * 結果は broadcast の結果の文字列 (`am broadcast` が `data="..."` として表示する) で返す。SET_CONFIG と STATUS は
 * 端末の識別子 (`device_id=...`、[DeviceIdentity]) も返す。
 * adb のシェル (DUMP の権限を持つ) からしか送れないよう、マニフェストで権限をかけてある。
 */
/**
 * Hilt は onReceive の中で注入するので、子クラスは super.onReceive を呼ぶ必要がある。BroadcastReceiver の onReceive は
 * 抽象メソッドで Kotlin からは呼べないため、空の実装を挟む。
 */
abstract class InjectableBroadcastReceiver : BroadcastReceiver() {
    @CallSuper
    override fun onReceive(context: Context, intent: Intent) = Unit
}

@AndroidEntryPoint
class RemoteControlReceiver : InjectableBroadcastReceiver() {
    @Inject
    lateinit var configRepository: ConfigStoreRepository

    @Inject
    lateinit var networkManager: NetworkManager

    @Inject
    lateinit var commands: RemoteCommands

    @Inject
    lateinit var deviceIdentity: DeviceIdentity

    @Inject
    @ApplicationScope
    lateinit var scope: CoroutineScope

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        val pending = goAsync()
        scope.launch {
            val result = try {
                handle(intent)
            } catch (e: Exception) {
                Timber.tag(TAG).w(e, "remote command failed: %s", intent.action)
                "error: ${e.message}"
            }
            Timber.tag(TAG).i("%s -> %s", intent.action, result)
            pending.resultData = result
            pending.finish()
        }
    }

    private suspend fun handle(intent: Intent): String = when (intent.action) {
        ACTION_SET_CONFIG -> setConfig(intent)
        ACTION_SAVE_FRAME -> when {
            !commands.cameraScreenActive -> "error: the camera screen is not open"
            commands.requestSaveFrame() -> "ok: save frame requested"
            else -> "error: a save is already pending"
        }

        ACTION_RETRY_CONNECTION -> {
            networkManager.retryConnection(scope)
            "ok: retrying connection"
        }

        ACTION_STATUS -> status()
        else -> "error: unknown action ${intent.action}"
    }

    private suspend fun setConfig(intent: Intent): String {
        val applied = mutableListOf<String>()
        intent.getStringExtra("output")?.let { text ->
            val kind = DetectionOutputKind.entries.firstOrNull { it.name.equals(text, ignoreCase = true) }
                ?: return "error: output must be one of ${DetectionOutputKind.entries}"
            configRepository.setDetectionOutputKind(kind)
            applied += "output=$kind"
        }
        intent.getStringExtra("focal_ratio")?.let { text ->
            val ratio = text.toDoubleOrNull()?.takeIf { it >= 0.0 } ?: return "error: invalid focal_ratio $text"
            configRepository.updateFocalLengthRatio(ratio)
            applied += "focal_ratio=$ratio"
        }
        intent.getStringExtra("pose")?.let { text ->
            val v = numbers(text, 6) ?: return "error: pose must be x,y,z,rx,ry,rz"
            configRepository.updateCameraPose(cameraPose {
                x = v[0]
                y = v[1]
                z = v[2]
                rotationXDegrees = v[3]
                rotationYDegrees = v[4]
                rotationZDegrees = v[5]
            })
            applied += "pose=${v.joinToString(",")}"
        }
        intent.getStringExtra("area")?.let { text ->
            val v = numbers(text, 4) ?: return "error: area must be left,top,right,bottom"
            if (!DetectionArea.isValid(v[0], v[1], v[2], v[3])) return "error: the area leaves nothing of the image"
            configRepository.updateDetectionArea(DetectionArea(v[0], v[1], v[2], v[3]))
            applied += "area=${v.joinToString(",")}"
        }
        // PC はこの device_id を scripts/cameras.json に書き、ball_tracker が端末を見分けるのに使う
        return if (applied.isEmpty()) "error: nothing to set" else
            "ok: " + applied.joinToString(" ") + " device_id=${deviceIdentity.deviceId}"
    }

    private suspend fun status(): String {
        val pose = configRepository.cameraPose.first()
        val area = configRepository.detectionArea.value
        return listOf(
            "device_id=${deviceIdentity.deviceId}",
            "output=${configRepository.detectionOutputKind.value}",
            "tcp=${if (networkManager.state.first().tcpIsConnected()) "connected" else "disconnected"}",
            "camera_screen=${commands.cameraScreenActive}",
            "focal_ratio=${configRepository.focalLengthRatio.value}",
            "pose=${pose.x},${pose.y},${pose.z},${pose.rotationXDegrees},${pose.rotationYDegrees},${pose.rotationZDegrees}",
            "area=${area.leftPercent},${area.topPercent},${area.rightPercent},${area.bottomPercent}",
        ).joinToString(" ")
    }

    private fun numbers(text: String, count: Int): List<Double>? =
        text.split(",").map { it.trim().toDoubleOrNull() ?: return null }.takeIf { it.size == count }

    companion object {
        private const val TAG = "RemoteControl"
        private const val PREFIX = "com.taichi765.struckoutCameraApp.remote."
        const val ACTION_SET_CONFIG = PREFIX + "SET_CONFIG"
        const val ACTION_SAVE_FRAME = PREFIX + "SAVE_FRAME"
        const val ACTION_RETRY_CONNECTION = PREFIX + "RETRY_CONNECTION"
        const val ACTION_STATUS = PREFIX + "STATUS"

        /** アプリを開くときに、開く画面を指定する (`am start ... --es screen camera`) */
        const val EXTRA_SCREEN = "screen"
    }
}
