package com.taichi765.struckoutCameraApp.recording

import androidx.room.Dao
import androidx.room.Database
import androidx.room.Entity
import androidx.room.Insert
import androidx.room.PrimaryKey
import androidx.room.RoomDatabase
import androidx.room.TypeConverter
import androidx.room.TypeConverters
import com.taichi765.struckoutCameraApp.proto.Struckout


@Entity(
    tableName = "frames"
)
data class FrameEntity(
    @PrimaryKey
    val timestamp: Long,

    val data: Struckout.UdpPacket
)

@Dao
interface FrameDao {
    @Insert
    suspend fun insertFrame(frame: FrameEntity)
}

@TypeConverters(Converters::class)
@Database(entities = [FrameEntity::class], version = 1)
abstract class AppDatabase : RoomDatabase() {
    abstract fun frameDao(): FrameDao
}

class Converters {
    @TypeConverter
    fun packetToBlob(packet: Struckout.UdpPacket): ByteArray {
        return packet.toByteArray()
    }

    @TypeConverter
    fun blobToPacket(blob: ByteArray): Struckout.UdpPacket =
        Struckout.UdpPacket.newBuilder().mergeFrom(blob).build()
}