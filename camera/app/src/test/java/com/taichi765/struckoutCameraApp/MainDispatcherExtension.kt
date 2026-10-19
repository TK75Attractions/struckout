package com.taichi765.struckoutCameraApp

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.jupiter.api.extension.AfterEachCallback
import org.junit.jupiter.api.extension.BeforeEachCallback
import org.junit.jupiter.api.extension.ExtensionContext

/**
 * テストのあいだ [Dispatchers.Main] を [StandardTestDispatcher] に差し替える。
 *
 * `viewModelScope` は [Dispatchers.Main] で動くが、JVM の単体テストには Main がないので
 * [Dispatchers.Default] にフォールバックする。そのままだと ViewModel のコルーチンが `runTest` の
 * 仮想時間の外で並行して走り、テストが終わったあとのモックの呼び出しが次のテストの失敗として報告される。
 * 差し替えておけば、`runTest` は同じスケジューラーを使う。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class MainDispatcherExtension : BeforeEachCallback, AfterEachCallback {
    override fun beforeEach(context: ExtensionContext) {
        Dispatchers.setMain(StandardTestDispatcher())
    }

    override fun afterEach(context: ExtensionContext) {
        Dispatchers.resetMain()
    }
}
