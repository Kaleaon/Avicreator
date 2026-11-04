package com.charmorph.app.ui

import android.content.Context
import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.workDataOf
import com.charmorph.ingest.workers.FeatureExtractionWorker
import com.charmorph.ingest.workers.FinalizeWorker
import com.charmorph.ingest.workers.MLFittingWorker
import com.charmorph.ingest.workers.ParseMeshWorker
import com.charmorph.ingest.workers.PrepareUploadsWorker
import com.charmorph.ingest.workers.SliderSynthesisWorker
import com.charmorph.ingest.workers.WorkerConstants
import com.charmorph.storage.CharacterRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

data class ImportUiState(
    val isProcessing: Boolean = false,
    val progress: Float = 0f,
    val statusText: String = "Waiting for file...",
    val isCompleted: Boolean = false,
    val characterId: String? = null,
    val error: String? = null
)

@HiltViewModel
class ImportViewModel @Inject constructor(
    private val repository: CharacterRepository,
    private val workManager: WorkManager,
    @ApplicationContext private val context: Context
) : ViewModel() {

    private val _uiState = MutableStateFlow(ImportUiState())
    val uiState: StateFlow<ImportUiState> = _uiState.asStateFlow()

    fun importTestCharacter() {
        startPipeline(uri = null, characterName = "Test Character")
    }

    fun importFromUri(uri: Uri, characterName: String = "Imported Character") {
        startPipeline(uri = uri, characterName = characterName)
    }

    fun startPipeline(uri: Uri?, characterName: String = "Imported Character") {
        _uiState.value = ImportUiState(
            isProcessing = true,
            progress = 0.05f,
            statusText = "Enqueuing pipeline..."
        )

        val prepareWorker = OneTimeWorkRequestBuilder<PrepareUploadsWorker>()
            .setInputData(
                workDataOf(
                    WorkerConstants.KEY_INPUT_URI to (uri?.toString() ?: ""),
                    WorkerConstants.KEY_CHARACTER_NAME to characterName
                )
            )
            .build()

        val parseWorker = OneTimeWorkRequestBuilder<ParseMeshWorker>().build()
        val featureWorker = OneTimeWorkRequestBuilder<FeatureExtractionWorker>().build()
        val mlWorker = OneTimeWorkRequestBuilder<MLFittingWorker>().build()
        val sliderWorker = OneTimeWorkRequestBuilder<SliderSynthesisWorker>().build()
        val finalizeWorker = OneTimeWorkRequestBuilder<FinalizeWorker>().build()

        workManager.beginWith(prepareWorker)
            .then(parseWorker)
            .then(featureWorker)
            .then(mlWorker)
            .then(sliderWorker)
            .then(finalizeWorker)
            .enqueue()

        observeWorkProgress(
            prepareId = prepareWorker.id,
            parseId = parseWorker.id,
            featureId = featureWorker.id,
            mlId = mlWorker.id,
            sliderId = sliderWorker.id,
            finalizeId = finalizeWorker.id
        )
    }

    private fun observeWorkProgress(
        prepareId: UUID,
        parseId: UUID,
        featureId: UUID,
        mlId: UUID,
        sliderId: UUID,
        finalizeId: UUID
    ) {
        val workerInfos = mapOf(
            prepareId to ("Preparing Uploads..." to 0.16f),
            parseId to ("Parsing Mesh..." to 0.33f),
            featureId to ("Extracting Features..." to 0.50f),
            mlId to ("Fitting ML Models..." to 0.66f),
            sliderId to ("Synthesizing Sliders..." to 0.83f),
            finalizeId to ("Finalizing Character..." to 1.0f)
        )

        workerInfos.forEach { (id, stageDetails) ->
            viewModelScope.launch {
                workManager.getWorkInfoByIdFlow(id).collect { workInfo ->
                    if (workInfo != null) {
                        when (workInfo.state) {
                            WorkInfo.State.RUNNING -> {
                                _uiState.value = _uiState.value.copy(
                                    isProcessing = true,
                                    progress = stageDetails.second,
                                    statusText = stageDetails.first
                                )
                            }
                            WorkInfo.State.SUCCEEDED -> {
                                if (id == finalizeId) {
                                    val charId = workInfo.outputData.getString(WorkerConstants.KEY_CHARACTER_ID)
                                    _uiState.value = ImportUiState(
                                        isProcessing = false,
                                        progress = 1.0f,
                                        statusText = "Done!",
                                        isCompleted = true,
                                        characterId = charId
                                    )
                                } else {
                                    _uiState.value = _uiState.value.copy(
                                        isProcessing = true,
                                        progress = stageDetails.second,
                                        statusText = stageDetails.first
                                    )
                                }
                            }
                            WorkInfo.State.FAILED -> {
                                val errorMsg = workInfo.outputData.getString(WorkerConstants.KEY_ERROR) ?: "Pipeline failed"
                                _uiState.value = ImportUiState(
                                    isProcessing = false,
                                    progress = 0f,
                                    statusText = "Error: $errorMsg",
                                    error = errorMsg
                                )
                            }
                            else -> {}
                        }
                    }
                }
            }
        }
    }
}
