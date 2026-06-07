package com.taichi765.struckoutCameraApp

import com.taichi765.struckoutCameraApp.transport.bytesToInt
import org.junit.Test
import java.nio.ByteBuffer

class ReadPacketUnitTest {
    @Test
    fun `data length is deserialized correctly`() {
        val bytes = byteArrayOf(208.toByte(), 7, 0, 0)
        val len = bytesToInt(bytes)
        assert(len == 2000)
    }

    @Test
    fun `data length is serialized correctly`() {
        val len = 2000
        val bytes = ByteBuffer.allocate(4).putInt(len)
        TODO()
    }
}