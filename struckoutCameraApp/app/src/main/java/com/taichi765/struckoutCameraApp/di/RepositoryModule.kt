package com.taichi765.struckoutCameraApp.di

import com.taichi765.struckoutCameraApp.transport.ConfiguredDetectionRepository
import com.taichi765.struckoutCameraApp.transport.DetectionRepository
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent

@InstallIn(SingletonComponent::class)
@Module
abstract class RepositoryModule {

    @Binds
    abstract fun bindDetectionRepository(configuredDetectionRepository: ConfiguredDetectionRepository): DetectionRepository
}