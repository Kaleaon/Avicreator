package com.charmorph.ingest

import android.content.Context
import android.net.Uri
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.Operation
import androidx.work.WorkManager
import androidx.work.workDataOf
import com.charmorph.ingest.workers.FeatureExtractionWorker
import com.charmorph.ingest.workers.FinalizeWorker
import com.charmorph.ingest.workers.MLFittingWorker
import com.charmorph.ingest.workers.ParseMeshWorker
import com.charmorph.ingest.workers.PipelineConstants
import com.charmorph.ingest.workers.PrepareUploadsWorker
import com.charmorph.ingest.workers.SliderSynthesisWorker
import java.util.UUID

class IngestPipelineManager(
    private val context: Context
) {
    fun enqueueIngestionPipeline(
        inputUri: Uri? = null,
        inputFilePath: String? = null,
        sessionId: String = UUID.randomUUID().toString()
    ): Pair<String, Operation> {
        val inputDataBuilder = workDataOf(
            PipelineConstants.KEY_SESSION_ID to sessionId,
            PipelineConstants.KEY_INPUT_URI to inputUri?.toString(),
            PipelineConstants.KEY_INPUT_PATH to inputFilePath
        )

        val prepareWorker = OneTimeWorkRequestBuilder<PrepareUploadsWorker>()
            .setInputData(inputDataBuilder)
            .build()

        val parseWorker = OneTimeWorkRequestBuilder<ParseMeshWorker>().build()
        val featureWorker = OneTimeWorkRequestBuilder<FeatureExtractionWorker>().build()
        val mlFittingWorker = OneTimeWorkRequestBuilder<MLFittingWorker>().build()
        val sliderWorker = OneTimeWorkRequestBuilder<SliderSynthesisWorker>().build()
        val finalizeWorker = OneTimeWorkRequestBuilder<FinalizeWorker>().build()

        val operation = WorkManager.getInstance(context)
            .beginWith(prepareWorker)
            .then(parseWorker)
            .then(featureWorker)
            .then(mlFittingWorker)
            .then(sliderWorker)
            .then(finalizeWorker)
            .enqueue()

        return Pair(sessionId, operation)
    }
}
