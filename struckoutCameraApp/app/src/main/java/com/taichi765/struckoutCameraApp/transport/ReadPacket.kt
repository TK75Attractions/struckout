package com.taichi765.struckoutCameraApp.transport

import com.google.protobuf.MessageLite
import java.io.InputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

inline fun <T : MessageLite> readPacket(
    input: InputStream,
    crossinline parser: (ByteArray) -> T
): T {
    val len = run {
        val bytes = ByteArray(4)
        input.readNBytes(bytes, 0, 4)
        bytesToInt(bytes)
    }
    val bytes = ByteArray(len)
    input.readNBytes(bytes, 0, len)
    return parser(bytes)
}

fun bytesToInt(bytes: ByteArray): Int {
    require(bytes.size == 4) {
        "the size of bytes must be 4 in order to convert to Int. Actual size was: ${bytes.size}"
    }
    return ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).getInt()
}