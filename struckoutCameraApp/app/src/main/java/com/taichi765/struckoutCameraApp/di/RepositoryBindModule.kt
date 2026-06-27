package com.taichi765.struckoutCameraApp.di

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepositoryImpl
import com.taichi765.struckoutCameraApp.transport.CameraLocationDataSource
import com.taichi765.struckoutCameraApp.transport.ConfiguredDetectionRepository
import com.taichi765.struckoutCameraApp.transport.DetectionRepository
import com.taichi765.struckoutCameraApp.transport.SessionStateProvider
import com.taichi765.struckoutCameraApp.transport.TcpSession
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent

@InstallIn(SingletonComponent::class)
@Module
abstract class RepositoryBindModule {

    @Binds
    abstract fun bindDetectionRepository(configuredDetectionRepository: ConfiguredDetectionRepository): DetectionRepository

    @Binds
    abstract fun bindCameraLocationDataSource(configStoreRepository: ConfigStoreRepositoryImpl): CameraLocationDataSource

    @Binds
    abstract fun bindSessionStateProvider(tcpSession: TcpSession): SessionStateProvider

    @Binds
    abstract fun bindConfigStoreRepository(configStoreRepository: ConfigStoreRepositoryImpl): ConfigStoreRepository
}