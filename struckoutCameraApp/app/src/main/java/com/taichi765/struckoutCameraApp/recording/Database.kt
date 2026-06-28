package com.taichi765.struckoutCameraApp.recording

import androidx.room.Dao
import androidx.room.Database
import androidx.room.Entity
import androidx.room.Insert
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.RoomDatabase
import androidx.room.TypeConverter
import androidx.room.TypeConverters
import com.taichi765.struckoutCameraApp.proto.Struckout
import kotlinx.coroutines.flow.Flow


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
    @Query("SELECT COUNT(*) FROM frames")
    fun countRows(): Flow<Int>

    @Insert
    suspend fun insertFrame(frame: FrameEntity)

    // TODO: Flowの方がいいかも
    @Query("SELECT * FROM frames")
    suspend fun loadAll(): List<FrameEntity>
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