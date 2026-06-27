package com.taichi765.struckoutCameraApp.di

import com.taichi765.struckoutCameraApp.config.ConfigStoreRepository
import com.taichi765.struckoutCameraApp.config.ConfigStoreRepositoryImpl
import com.taichi765.struckoutCameraApp.network.CameraLocationDataSource
import com.taichi765.struckoutCameraApp.network.ConfiguredDetectionRepository
import com.taichi765.struckoutCameraApp.network.DetectionRepository
import com.taichi765.struckoutCameraApp.network.SessionStateProvider
import com.taichi765.struckoutCameraApp.network.TcpSession
import com.taichi765.struckoutCameraApp.network.TcpSessionImpl
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
    abstract fun bindSessionStateProvider(tcpSession: TcpSessionImpl): SessionStateProvider

    @Binds
    abstract fun bindConfigStoreRepository(configStoreRepository: ConfigStoreRepositoryImpl): ConfigStoreRepository

    @Binds
    abstract fun bindTcpSessionFactory(tcpSessionFactory: TcpSessionImpl.Factory): TcpSession.Factory
}