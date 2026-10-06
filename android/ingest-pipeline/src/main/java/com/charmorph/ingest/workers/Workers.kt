package com.charmorph.ingest.workers

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import kotlinx.coroutines.delay

class PrepareUploadsWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        // TODO: Copy file from URI to staging directory
        delay(500)
        return Result.success()
    }
}

class ParseMeshWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        // TODO: Use AssetImporter (via JNI) to parse the mesh
        delay(500)
        return Result.success()
    }
}

class FeatureExtractionWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        // TODO: Extract features for ML
        delay(500)
        return Result.success()
    }
}

class MLFittingWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        // TODO: Run TFLite models
        delay(1000)
        return Result.success()
    }
}

class SliderSynthesisWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        // TODO: Generate slider definitions based on ML output
        delay(500)
        return Result.success()
    }
}

class FinalizeWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        // TODO: Clean up and save results to DB
        delay(200)
        return Result.success()
    }
}
