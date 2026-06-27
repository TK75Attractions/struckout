package com.taichi765.struckoutCameraApp.di

import com.taichi765.struckoutCameraApp.network.UdpConnection
import com.taichi765.struckoutCameraApp.network.UdpConnectionImpl
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent

@InstallIn(SingletonComponent::class)
@Module
object RepositoryProvideModule {
    @Provides
    fun provideUdpConnectionFactory(): UdpConnection.Factory = UdpConnectionImpl.Factory
}