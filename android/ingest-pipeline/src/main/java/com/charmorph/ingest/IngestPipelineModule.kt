package com.charmorph.ingest

import android.content.Context
import androidx.work.WorkManager
import com.charmorph.nativebridge.NativeLib
import dagger.Binds
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
abstract class IngestPipelineModule {

    @Binds
    @Singleton
    abstract fun bindAssetImporter(impl: DefaultAssetImporter): AssetImporter

    companion object {
        @Provides
        @Singleton
        fun provideWorkManager(@ApplicationContext context: Context): WorkManager {
            return WorkManager.getInstance(context)
        }

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
}
