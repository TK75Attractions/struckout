package com.taichi765.struckoutCameraApp.di

import com.taichi765.struckoutCameraApp.network.DataConnection
import com.taichi765.struckoutCameraApp.network.DataConnectionImpl
import com.taichi765.struckoutCameraApp.network.LocalDetectionUploader
import com.taichi765.struckoutCameraApp.network.LocalDetectionUploaderImpl
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent

@InstallIn(SingletonComponent::class)
@Module
object RepositoryProvideModule {
    @Provides
    fun provideUdpConnectionFactory(): DataConnection.Factory = DataConnectionImpl.Factory

    @Provides
    fun provideTcpSynchronizerFactory(): LocalDetectionUploader.Factory =
        LocalDetectionUploaderImpl.Factory
}