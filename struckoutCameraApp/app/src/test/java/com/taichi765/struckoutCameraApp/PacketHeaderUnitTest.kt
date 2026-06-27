package com.taichi765.struckoutCameraApp

import com.taichi765.struckoutCameraApp.network.bytesToInt
import org.junit.jupiter.api.Test
import java.nio.ByteBuffer
import java.nio.ByteOrder

class PacketHeaderUnitTest {
    @Test
    fun `data length is deserialized correctly`() {
        val bytes = byteArrayOf(208.toByte(), 7, 0, 0)
        val len = bytesToInt(bytes)
        assert(len == 2000)
    }

    @Test
    fun `data length is serialized correctly`() {
        val len = 2000
        val bytes = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(len)
        assert(bytes.array().contentEquals(byteArrayOf(-48, 7, 0, 0)))
    }
}