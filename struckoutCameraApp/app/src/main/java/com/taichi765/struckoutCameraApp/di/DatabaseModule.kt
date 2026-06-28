package com.taichi765.struckoutCameraApp.di

import android.content.Context
import androidx.room.Room
import com.taichi765.struckoutCameraApp.recording.AppDatabase
import com.taichi765.struckoutCameraApp.recording.FrameDao
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@InstallIn(SingletonComponent::class)
@Module
object DatabaseModule {
    @Provides
    @Singleton
    fun provideAppDatabase(@ApplicationContext context: Context): AppDatabase =
        Room.databaseBuilder(
            context = context,
            klass = AppDatabase::class.java,
            name = "app-database"
        ).build()

    @Provides
    fun provideFrameDao(database: AppDatabase): FrameDao =
        database.frameDao()
}