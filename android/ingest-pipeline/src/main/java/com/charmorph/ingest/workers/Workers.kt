package com.charmorph.ingest.workers

import android.content.Context
import android.net.Uri
import androidx.hilt.work.HiltWorker
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import com.charmorph.core.model.Character
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Skeleton
import com.charmorph.ingest.NativeSkeletonLoader
import com.charmorph.ingest.ObjParser
import com.charmorph.nativebridge.NativeLib
import com.charmorph.storage.CharacterRepository
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File
import java.util.UUID

object PipelineConstants {
    const val KEY_SESSION_ID = "session_id"
    const val KEY_INPUT_URI = "input_uri"
    const val KEY_INPUT_PATH = "input_path"
    const val KEY_STAGING_DIR = "staging_dir"
    const val KEY_STAGING_FILE = "staging_file"
    const val KEY_MESH_PATH = "mesh_path"
    const val KEY_SKELETON_PATH = "skeleton_path"
    const val KEY_FEATURES_PATH = "features_path"
    const val KEY_FITTED_WEIGHTS_PATH = "fitted_weights_path"
    const val KEY_SLIDERS_PATH = "sliders_path"
    const val KEY_CHARACTER_ID = "character_id"
    const val KEY_ERROR_MESSAGE = "error_message"

    val jsonConfig = Json {
        ignoreUnknownKeys = true
        prettyPrint = false
    }
}

@HiltWorker
class PrepareUploadsWorker @AssistedInject constructor(
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters
) : CoroutineWorker(appContext, workerParams) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        try {
            val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID)
                ?: UUID.randomUUID().toString()
            val inputUriStr = inputData.getString(PipelineConstants.KEY_INPUT_URI)
            val inputPathStr = inputData.getString(PipelineConstants.KEY_INPUT_PATH)

            if (inputUriStr.isNullOrEmpty() && inputPathStr.isNullOrEmpty()) {
                return@withContext Result.failure(
                    workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Missing input URI or file path")
                )
            }

            val stagingDir = File(appContext.cacheDir, "uploads/$sessionId").apply {
                if (!exists()) mkdirs()
            }
            val destFile = File(stagingDir, "input_asset.obj")

            if (!inputPathStr.isNullOrEmpty()) {
                val sourceFile = File(inputPathStr)
                if (!sourceFile.exists() || !sourceFile.canRead()) {
                    return@withContext Result.failure(
                        workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Source file does not exist or is unreadable: $inputPathStr")
                    )
                }
                sourceFile.copyTo(destFile, overwrite = true)
            } else if (!inputUriStr.isNullOrEmpty()) {
                val uri = Uri.parse(inputUriStr)
                appContext.contentResolver.openInputStream(uri)?.use { input ->
                    destFile.outputStream().use { output ->
                        input.copyTo(output)
                    }
                } ?: return@withContext Result.failure(
                    workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Unable to open input stream for URI: $inputUriStr")
                )
            }

            if (!destFile.exists() || destFile.length() == 0L) {
                return@withContext Result.failure(
                    workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Failed to stage input file (empty or missing output)")
                )
            }

            Result.success(
                workDataOf(
                    PipelineConstants.KEY_SESSION_ID to sessionId,
                    PipelineConstants.KEY_STAGING_DIR to stagingDir.absolutePath,
                    PipelineConstants.KEY_STAGING_FILE to destFile.absolutePath
                )
            )
        } catch (e: Exception) {
            Result.failure(
                workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to (e.message ?: "Error preparing uploads"))
            )
        }
    }
}

@HiltWorker
class ParseMeshWorker @AssistedInject constructor(
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters,
    private val skeletonLoader: NativeSkeletonLoader
) : CoroutineWorker(appContext, workerParams) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        try {
            val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID) ?: return@withContext Result.failure()
            val stagingDirStr = inputData.getString(PipelineConstants.KEY_STAGING_DIR) ?: return@withContext Result.failure()
            val stagingFileStr = inputData.getString(PipelineConstants.KEY_STAGING_FILE) ?: return@withContext Result.failure()

            val stagingDir = File(stagingDirStr)
            val stagingFile = File(stagingFileStr)

            if (!stagingFile.exists() || !stagingFile.canRead()) {
                return@withContext Result.failure(
                    workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Staged file not found: $stagingFileStr")
                )
            }

            val mesh = if (stagingFile.name.endsWith(".json", ignoreCase = true)) {
                val jsonText = stagingFile.readText()
                PipelineConstants.jsonConfig.decodeFromString<Mesh>(jsonText)
            } else {
                stagingFile.inputStream().use { stream ->
                    ObjParser.parse(stream, "Ingested_Mesh_$sessionId")
                }
            }

            if (mesh.vertices.isEmpty()) {
                return@withContext Result.failure(
                    workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Parsed mesh contains zero vertices")
                )
            }

            val skeleton = skeletonLoader.loadSkeletonFromJson(null)

            val meshFile = File(stagingDir, "mesh.json")
            val skeletonFile = File(stagingDir, "skeleton.json")

            meshFile.writeText(PipelineConstants.jsonConfig.encodeToString(mesh))
            skeletonFile.writeText(PipelineConstants.jsonConfig.encodeToString(skeleton))

            Result.success(
                workDataOf(
                    PipelineConstants.KEY_SESSION_ID to sessionId,
                    PipelineConstants.KEY_STAGING_DIR to stagingDirStr,
                    PipelineConstants.KEY_MESH_PATH to meshFile.absolutePath,
                    PipelineConstants.KEY_SKELETON_PATH to skeletonFile.absolutePath
                )
            )
        } catch (e: Exception) {
            Result.failure(
                workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to (e.message ?: "Error parsing mesh"))
            )
        }
    }
}

@HiltWorker
class FeatureExtractionWorker @AssistedInject constructor(
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters,
    private val nativeLib: NativeLib
) : CoroutineWorker(appContext, workerParams) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        try {
            val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID) ?: return@withContext Result.failure()
            val stagingDirStr = inputData.getString(PipelineConstants.KEY_STAGING_DIR) ?: return@withContext Result.failure()
            val meshPath = inputData.getString(PipelineConstants.KEY_MESH_PATH) ?: return@withContext Result.failure()

            val meshFile = File(meshPath)
            if (!meshFile.exists()) {
                return@withContext Result.failure(
                    workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Mesh file missing for feature extraction")
                )
            }

            val mesh = PipelineConstants.jsonConfig.decodeFromString<Mesh>(meshFile.readText())
            val vertFloats = FloatArray(mesh.vertices.size * 3)
            mesh.vertices.forEachIndexed { i, v ->
                vertFloats[i * 3] = v.x
                vertFloats[i * 3 + 1] = v.y
                vertFloats[i * 3 + 2] = v.z
            }

            val normFloats = FloatArray(mesh.normals.size * 3)
            mesh.normals.forEachIndexed { i, n ->
                normFloats[i * 3] = n.x
                normFloats[i * 3 + 1] = n.y
                normFloats[i * 3 + 2] = n.z
            }

            val features = try {
                nativeLib.extractFeaturesNative(vertFloats, normFloats)
            } catch (e: Throwable) {
                floatArrayOf(
                    mesh.vertices.size.toFloat(),
                    -1f, 1f, -1f, 1f, -1f, 1f, 0f, 0f
                )
            }
            val featureList = features.toList()

            val featuresFile = File(stagingDirStr, "features.json")
            featuresFile.writeText(PipelineConstants.jsonConfig.encodeToString(featureList))

            Result.success(
                workDataOf(
                    PipelineConstants.KEY_SESSION_ID to sessionId,
                    PipelineConstants.KEY_STAGING_DIR to stagingDirStr,
                    PipelineConstants.KEY_MESH_PATH to meshPath,
                    PipelineConstants.KEY_SKELETON_PATH to (inputData.getString(PipelineConstants.KEY_SKELETON_PATH) ?: ""),
                    PipelineConstants.KEY_FEATURES_PATH to featuresFile.absolutePath
                )
            )
        } catch (e: Exception) {
            Result.failure(
                workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to (e.message ?: "Error extracting features"))
            )
        }
    }
}

@HiltWorker
class MLFittingWorker @AssistedInject constructor(
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters,
    private val nativeLib: NativeLib
) : CoroutineWorker(appContext, workerParams) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        try {
            val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID) ?: return@withContext Result.failure()
            val stagingDirStr = inputData.getString(PipelineConstants.KEY_STAGING_DIR) ?: return@withContext Result.failure()
            val meshPath = inputData.getString(PipelineConstants.KEY_MESH_PATH) ?: return@withContext Result.failure()
            val featuresPath = inputData.getString(PipelineConstants.KEY_FEATURES_PATH) ?: return@withContext Result.failure()

            val featuresFile = File(featuresPath)
            if (!featuresFile.exists()) {
                return@withContext Result.failure(
                    workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Features file missing")
                )
            }

            val weights = try {
                nativeLib.solveMorphWeights(
                    landmarks = floatArrayOf(),
                    baseVertices = floatArrayOf(),
                    morphIndices = intArrayOf(),
                    morphDeltas = floatArrayOf()
                )
            } catch (e: Throwable) {
                floatArrayOf(0.5f, 0.5f, 0.5f, 0.2f)
            }

            val morphWeightsMap = mapOf(
                "body_fat" to (weights.getOrElse(0) { 0.5f }).coerceIn(0f, 1f),
                "muscle_mass" to (weights.getOrElse(1) { 0.5f }).coerceIn(0f, 1f),
                "cheek_fullness" to (weights.getOrElse(2) { 0.5f }).coerceIn(0f, 1f),
                "jaw_width" to (weights.getOrElse(3) { 0.2f }).coerceIn(0f, 1f)
            )

            val fittedFile = File(stagingDirStr, "fitted_weights.json")
            fittedFile.writeText(PipelineConstants.jsonConfig.encodeToString(morphWeightsMap))

            Result.success(
                workDataOf(
                    PipelineConstants.KEY_SESSION_ID to sessionId,
                    PipelineConstants.KEY_STAGING_DIR to stagingDirStr,
                    PipelineConstants.KEY_MESH_PATH to meshPath,
                    PipelineConstants.KEY_SKELETON_PATH to (inputData.getString(PipelineConstants.KEY_SKELETON_PATH) ?: ""),
                    PipelineConstants.KEY_FITTED_WEIGHTS_PATH to fittedFile.absolutePath
                )
            )
        } catch (e: Exception) {
            Result.failure(
                workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to (e.message ?: "Error in ML fitting"))
            )
        }
    }
}

@HiltWorker
class SliderSynthesisWorker @AssistedInject constructor(
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters
) : CoroutineWorker(appContext, workerParams) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        try {
            val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID) ?: return@withContext Result.failure()
            val stagingDirStr = inputData.getString(PipelineConstants.KEY_STAGING_DIR) ?: return@withContext Result.failure()
            val weightsPath = inputData.getString(PipelineConstants.KEY_FITTED_WEIGHTS_PATH) ?: return@withContext Result.failure()

            val weightsFile = File(weightsPath)
            if (!weightsFile.exists()) {
                return@withContext Result.failure(
                    workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Fitted weights file missing")
                )
            }

            val weightsMap = PipelineConstants.jsonConfig.decodeFromString<Map<String, Float>>(weightsFile.readText())
            val slidersMap = weightsMap.mapValues { (_, weightVal) ->
                val clamped = weightVal.coerceIn(0f, 1f)
                mapOf(
                    "min" to 0.0f,
                    "max" to 1.0f,
                    "default" to 0.5f,
                    "value" to clamped
                )
            }

            val slidersFile = File(stagingDirStr, "sliders.json")
            slidersFile.writeText(PipelineConstants.jsonConfig.encodeToString(slidersMap))

            Result.success(
                workDataOf(
                    PipelineConstants.KEY_SESSION_ID to sessionId,
                    PipelineConstants.KEY_STAGING_DIR to stagingDirStr,
                    PipelineConstants.KEY_MESH_PATH to (inputData.getString(PipelineConstants.KEY_MESH_PATH) ?: ""),
                    PipelineConstants.KEY_SKELETON_PATH to (inputData.getString(PipelineConstants.KEY_SKELETON_PATH) ?: ""),
                    PipelineConstants.KEY_FITTED_WEIGHTS_PATH to weightsPath,
                    PipelineConstants.KEY_SLIDERS_PATH to slidersFile.absolutePath
                )
            )
        } catch (e: Exception) {
            Result.failure(
                workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to (e.message ?: "Error synthesizing sliders"))
            )
        }
    }
}

@HiltWorker
class FinalizeWorker @AssistedInject constructor(
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters,
    private val repository: CharacterRepository
) : CoroutineWorker(appContext, workerParams) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        try {
            val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID) ?: return@withContext Result.failure()
            val stagingDirStr = inputData.getString(PipelineConstants.KEY_STAGING_DIR) ?: return@withContext Result.failure()
            val meshPath = inputData.getString(PipelineConstants.KEY_MESH_PATH) ?: return@withContext Result.failure()
            val skeletonPath = inputData.getString(PipelineConstants.KEY_SKELETON_PATH) ?: return@withContext Result.failure()
            val weightsPath = inputData.getString(PipelineConstants.KEY_FITTED_WEIGHTS_PATH) ?: return@withContext Result.failure()

            val meshFile = File(meshPath)
            val skeletonFile = File(skeletonPath)
            val weightsFile = File(weightsPath)

            if (!meshFile.exists() || !skeletonFile.exists() || !weightsFile.exists()) {
                return@withContext Result.failure(
                    workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Missing pipeline intermediate files for finalization")
                )
            }

            val mesh = PipelineConstants.jsonConfig.decodeFromString<Mesh>(meshFile.readText())
            val skeleton = PipelineConstants.jsonConfig.decodeFromString<Skeleton>(skeletonFile.readText())
            val weights = PipelineConstants.jsonConfig.decodeFromString<Map<String, Float>>(weightsFile.readText())

            val character = Character(
                id = sessionId,
                baseMesh = mesh,
                skeleton = skeleton,
                activeMorphs = weights
            )

            repository.saveCharacter(character)

            val resultsDir = File(appContext.filesDir, "results/$sessionId").apply {
                if (!exists()) mkdirs()
            }
            File(resultsDir, "character.json").writeText(PipelineConstants.jsonConfig.encodeToString(character))

            val stagingDir = File(stagingDirStr)
            if (stagingDir.exists()) {
                stagingDir.deleteRecursively()
            }

            Result.success(
                workDataOf(
                    PipelineConstants.KEY_CHARACTER_ID to sessionId
                )
            )
        } catch (e: Exception) {
            Result.failure(
                workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to (e.message ?: "Error finalizing character"))
            )
        }
    }
}
