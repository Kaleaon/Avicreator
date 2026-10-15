package com.charmorph.ingest

import com.charmorph.nativebridge.NativeLib
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object IngestPipelineModule {

    @Provides
    @Singleton
    fun provideNativeLib(): NativeLib {
        return NativeLib()
    }

    @Provides
    @Singleton
    fun provideNativeSkeletonLoader(nativeLib: NativeLib): NativeSkeletonLoader {
        return NativeSkeletonLoader(nativeLib)
    }
}
