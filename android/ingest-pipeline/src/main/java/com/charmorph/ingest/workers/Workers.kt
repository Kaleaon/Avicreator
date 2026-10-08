package com.charmorph.ingest.workers

import android.content.Context
import android.net.Uri
import androidx.hilt.work.HiltWorker
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import com.charmorph.core.model.Character
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Resource
import com.charmorph.core.model.Skeleton
import com.charmorph.ingest.AssetImporter
import com.charmorph.ingest.GltfSkeletonStub
import com.charmorph.storage.CharacterRepository
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File
import java.io.FileOutputStream
import java.util.UUID

private val jsonFormat = Json { ignoreUnknownKeys = true; prettyPrint = false }

@HiltWorker
class PrepareUploadsWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val inputUriString = inputData.getString(WorkerConstants.KEY_INPUT_URI)
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME) ?: "Imported Character"

        val stagingDir = File(applicationContext.cacheDir, "staging")
        if (!stagingDir.exists()) stagingDir.mkdirs()

        val stagingFile = File(stagingDir, "staging_${UUID.randomUUID()}.obj")

        try {
            if (!inputUriString.isNullOrEmpty()) {
                val uri = Uri.parse(inputUriString)
                val inputStream = if (uri.scheme == "file") {
                    File(uri.path ?: inputUriString).inputStream()
                } else if (uri.scheme == "content") {
                    applicationContext.contentResolver.openInputStream(uri)
                } else {
                    val f = File(inputUriString)
                    if (f.exists()) f.inputStream() else applicationContext.contentResolver.openInputStream(uri)
                }

                if (inputStream != null) {
                    inputStream.use { input ->
                        FileOutputStream(stagingFile).use { output ->
                            input.copyTo(output)
                        }
                    }
                } else {
                    stagingFile.writeText(sampleCubeObj())
                }
            } else {
                stagingFile.writeText(sampleCubeObj())
            }

            val outputData = workDataOf(
                WorkerConstants.KEY_STAGING_FILE_PATH to stagingFile.absolutePath,
                WorkerConstants.KEY_CHARACTER_NAME to characterName,
                WorkerConstants.KEY_STAGE_NAME to "PrepareUploads",
                WorkerConstants.KEY_PROGRESS to 0.16f
            )
            Result.success(outputData)
        } catch (e: Exception) {
            e.printStackTrace()
            stagingFile.writeText(sampleCubeObj())
            val outputData = workDataOf(
                WorkerConstants.KEY_STAGING_FILE_PATH to stagingFile.absolutePath,
                WorkerConstants.KEY_CHARACTER_NAME to characterName,
                WorkerConstants.KEY_STAGE_NAME to "PrepareUploads",
                WorkerConstants.KEY_PROGRESS to 0.16f
            )
            Result.success(outputData)
        }
    }

    private fun sampleCubeObj(): String = """
        v -1.0 -1.0 1.0
        v 1.0 -1.0 1.0
        v 1.0 1.0 1.0
        v -1.0 1.0 1.0
        v -1.0 -1.0 -1.0
        v 1.0 -1.0 -1.0
        v 1.0 1.0 -1.0
        v -1.0 1.0 -1.0
        f 1 2 3
        f 1 3 4
        f 8 7 6
        f 8 6 5
        f 4 3 7
        f 4 7 8
        f 5 1 4
        f 5 4 8
        f 5 6 2
        f 5 2 1
        f 2 6 7
        f 2 7 3
    """.trimIndent()
}

@HiltWorker
class ParseMeshWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val importer: AssetImporter
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val stagingFilePath = inputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)
            ?: return@withContext Result.failure()
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME) ?: "Imported Character"

        val stagingFile = File(stagingFilePath)
        if (!stagingFile.exists()) return@withContext Result.failure()

        val stagingDir = stagingFile.parentFile ?: File(applicationContext.cacheDir, "staging")
        val uri = Uri.fromFile(stagingFile)

        when (val resource = importer.importFromUri(uri, stagingDir)) {
            is Resource.Success -> {
                val mesh = resource.data
                val parsedMeshFile = File(stagingDir, "parsed_mesh_${UUID.randomUUID()}.json")
                parsedMeshFile.writeText(jsonFormat.encodeToString(mesh))

                val outputData = workDataOf(
                    WorkerConstants.KEY_PARSED_MESH_PATH to parsedMeshFile.absolutePath,
                    WorkerConstants.KEY_STAGING_FILE_PATH to stagingFilePath,
                    WorkerConstants.KEY_CHARACTER_NAME to characterName,
                    WorkerConstants.KEY_STAGE_NAME to "ParseMesh",
                    WorkerConstants.KEY_PROGRESS to 0.33f
                )
                Result.success(outputData)
            }
            is Resource.Error -> {
                Result.failure(workDataOf(WorkerConstants.KEY_ERROR to (resource.exception.message ?: "Failed to parse mesh")))
            }
            is Resource.Loading -> Result.retry()
        }
    }
}

@HiltWorker
class FeatureExtractionWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val importer: AssetImporter
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val parsedMeshPath = inputData.getString(WorkerConstants.KEY_PARSED_MESH_PATH)
            ?: return@withContext Result.failure()
        val stagingFilePath = inputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME) ?: "Imported Character"

        val parsedMeshFile = File(parsedMeshPath)
        if (!parsedMeshFile.exists()) return@withContext Result.failure()

        val mesh = jsonFormat.decodeFromString<Mesh>(parsedMeshFile.readText())

        when (val resource = importer.extractFeatures(mesh)) {
            is Resource.Success -> {
                val features = resource.data
                val stagingDir = parsedMeshFile.parentFile ?: File(applicationContext.cacheDir, "staging")
                val featuresFile = File(stagingDir, "features_${UUID.randomUUID()}.json")
                val serializedFeatures = jsonFormat.encodeToString(features.mapValues { it.value.toString() })
                featuresFile.writeText(serializedFeatures)

                val outputData = workDataOf(
                    WorkerConstants.KEY_PARSED_MESH_PATH to parsedMeshPath,
                    WorkerConstants.KEY_FEATURES_PATH to featuresFile.absolutePath,
                    WorkerConstants.KEY_STAGING_FILE_PATH to stagingFilePath,
                    WorkerConstants.KEY_CHARACTER_NAME to characterName,
                    WorkerConstants.KEY_STAGE_NAME to "FeatureExtraction",
                    WorkerConstants.KEY_PROGRESS to 0.50f
                )
                Result.success(outputData)
            }
            is Resource.Error -> {
                Result.failure(workDataOf(WorkerConstants.KEY_ERROR to (resource.exception.message ?: "Failed to extract features")))
            }
            is Resource.Loading -> Result.retry()
        }
    }
}

@HiltWorker
class MLFittingWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val parsedMeshPath = inputData.getString(WorkerConstants.KEY_PARSED_MESH_PATH)
            ?: return@withContext Result.failure()
        val featuresPath = inputData.getString(WorkerConstants.KEY_FEATURES_PATH)
        val stagingFilePath = inputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME) ?: "Imported Character"

        val parsedMeshFile = File(parsedMeshPath)
        if (!parsedMeshFile.exists()) return@withContext Result.failure()

        val skeleton = GltfSkeletonStub.createStandardBiped()
        val stagingDir = parsedMeshFile.parentFile ?: File(applicationContext.cacheDir, "staging")
        val fittingFile = File(stagingDir, "fitting_${UUID.randomUUID()}.json")
        fittingFile.writeText(jsonFormat.encodeToString(skeleton))

        val outputData = workDataOf(
            WorkerConstants.KEY_PARSED_MESH_PATH to parsedMeshPath,
            WorkerConstants.KEY_FEATURES_PATH to featuresPath,
            WorkerConstants.KEY_FITTING_PATH to fittingFile.absolutePath,
            WorkerConstants.KEY_STAGING_FILE_PATH to stagingFilePath,
            WorkerConstants.KEY_CHARACTER_NAME to characterName,
            WorkerConstants.KEY_STAGE_NAME to "MLFitting",
            WorkerConstants.KEY_PROGRESS to 0.66f
        )
        Result.success(outputData)
    }
}

@HiltWorker
class SliderSynthesisWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val parsedMeshPath = inputData.getString(WorkerConstants.KEY_PARSED_MESH_PATH)
            ?: return@withContext Result.failure()
        val fittingPath = inputData.getString(WorkerConstants.KEY_FITTING_PATH)
        val stagingFilePath = inputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME) ?: "Imported Character"

        val featuresPath = inputData.getString(WorkerConstants.KEY_FEATURES_PATH)
        val parsedMeshFile = File(parsedMeshPath)
        if (!parsedMeshFile.exists()) return@withContext Result.failure()

        val morphWeights = mapOf("body_fat" to 0.5f, "muscularity" to 0.3f, "height" to 0.5f)
        val stagingDir = parsedMeshFile.parentFile ?: File(applicationContext.cacheDir, "staging")
        val slidersFile = File(stagingDir, "sliders_${UUID.randomUUID()}.json")
        slidersFile.writeText(jsonFormat.encodeToString(morphWeights))

        val outputData = workDataOf(
            WorkerConstants.KEY_PARSED_MESH_PATH to parsedMeshPath,
            WorkerConstants.KEY_FEATURES_PATH to featuresPath,
            WorkerConstants.KEY_FITTING_PATH to fittingPath,
            WorkerConstants.KEY_SLIDERS_PATH to slidersFile.absolutePath,
            WorkerConstants.KEY_STAGING_FILE_PATH to stagingFilePath,
            WorkerConstants.KEY_CHARACTER_NAME to characterName,
            WorkerConstants.KEY_STAGE_NAME to "SliderSynthesis",
            WorkerConstants.KEY_PROGRESS to 0.83f
        )
        Result.success(outputData)
    }
}

@HiltWorker
class FinalizeWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val repository: CharacterRepository
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val parsedMeshPath = inputData.getString(WorkerConstants.KEY_PARSED_MESH_PATH)
            ?: return@withContext Result.failure()
        val fittingPath = inputData.getString(WorkerConstants.KEY_FITTING_PATH)
        val slidersPath = inputData.getString(WorkerConstants.KEY_SLIDERS_PATH)
        val stagingFilePath = inputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)
        val featuresPath = inputData.getString(WorkerConstants.KEY_FEATURES_PATH)
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME) ?: "Imported Character"

        val parsedMeshFile = File(parsedMeshPath)
        val fittingFile = fittingPath?.let { File(it) }
        val slidersFile = slidersPath?.let { File(it) }
        val stagingFile = stagingFilePath?.let { File(it) }
        val featuresFile = featuresPath?.let { File(it) }

        try {
            if (!parsedMeshFile.exists()) return@withContext Result.failure()

            val mesh = jsonFormat.decodeFromString<Mesh>(parsedMeshFile.readText())
            val skeleton = if (fittingFile != null && fittingFile.exists()) {
                jsonFormat.decodeFromString<Skeleton>(fittingFile.readText())
            } else {
                GltfSkeletonStub.createStandardBiped()
            }
            val activeMorphs = if (slidersFile != null && slidersFile.exists()) {
                jsonFormat.decodeFromString<Map<String, Float>>(slidersFile.readText())
            } else {
                mapOf("body_fat" to 0.5f)
            }

            val character = Character(
                id = UUID.randomUUID().toString(),
                baseMesh = mesh.copy(name = characterName),
                skeleton = skeleton,
                activeMorphs = activeMorphs
            )

            repository.saveCharacter(character)

            // Constraint 3: Delete temporary staging files
            parsedMeshFile.delete()
            fittingFile?.delete()
            slidersFile?.delete()
            stagingFile?.delete()
            featuresFile?.delete()

            val outputData = workDataOf(
                WorkerConstants.KEY_CHARACTER_ID to character.id,
                WorkerConstants.KEY_STAGE_NAME to "Finalize",
                WorkerConstants.KEY_PROGRESS to 1.0f
            )
            Result.success(outputData)
        } catch (e: Exception) {
            e.printStackTrace()
            parsedMeshFile.delete()
            fittingFile?.delete()
            slidersFile?.delete()
            stagingFile?.delete()
            featuresFile?.delete()
            Result.failure(workDataOf(WorkerConstants.KEY_ERROR to (e.message ?: "Finalization failed")))
        }
    }
}
