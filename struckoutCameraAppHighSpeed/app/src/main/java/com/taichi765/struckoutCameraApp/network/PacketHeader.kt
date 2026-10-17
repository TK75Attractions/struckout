package com.taichi765.struckoutCameraApp.network

import androidx.annotation.CheckResult
import com.google.protobuf.MessageLite
import com.taichi765.struckoutCameraApp.InputStreamCompat
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.EOFException
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

suspend inline fun <T : MessageLite> readPacket(
    input: InputStream,
    crossinline parser: (ByteArray) -> T
): T {
    // 相手が切ると readNBytes は短い配列を返す。切れたことは IOException で知らせる
    // (adb reverse は、PC で誰も待ち受けていなくても接続をいったん受けてすぐ切る)
    val len = withContext(Dispatchers.IO) {
        val header = InputStreamCompat.readNBytes(input, 4)
        if (header.size < 4) throw EOFException("connection closed")
        bytesToInt(header)
    }
    if (len < 0) throw IOException("invalid packet length $len")

    val bytes = withContext(Dispatchers.IO) {
        InputStreamCompat.readNBytes(input, len)
    }
    if (bytes.size < len) throw EOFException("connection closed")
    return parser(bytes)
}

@CheckResult
fun bytesToInt(bytes: ByteArray): Int {
    require(bytes.size == 4) {
        "the size of bytes must be 4 in order to convert to Int. Actual size was: ${bytes.size}"
    }
    return ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).getInt()
}

suspend fun <P : MessageLite> writePacket(output: OutputStream, packet: P) {
    withContext(Dispatchers.IO) {
        val packetBytes = packet.toByteArray()
        val len = packetBytes.size
        val buf = ByteBuffer.allocate(4 + len).order(ByteOrder.LITTLE_ENDIAN)
        buf.putInt(len)
        buf.put(packetBytes)
        output.write(buf.array())
    }
}