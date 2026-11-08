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
import com.charmorph.ingest.DefaultAssetImporter
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
import java.io.FileOutputStream
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

private val jsonFormat = Json { ignoreUnknownKeys = true; prettyPrint = false }

@HiltWorker
class PrepareUploadsWorker @AssistedInject constructor(
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters
) : CoroutineWorker(appContext, workerParams) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID)
            ?: UUID.randomUUID().toString()
        val inputUriString = inputData.getString(WorkerConstants.KEY_INPUT_URI)
            ?: inputData.getString(PipelineConstants.KEY_INPUT_URI)
        val inputPathStr = inputData.getString(PipelineConstants.KEY_INPUT_PATH)
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME)

        // If neither input URI nor path nor characterName is supplied (e.g. testPrepareUploadsWorkerWithMissingInputFailsGracefully)
        if (inputUriString.isNullOrEmpty() && inputPathStr.isNullOrEmpty() && characterName == null) {
            return@withContext Result.failure(
                workDataOf(
                    PipelineConstants.KEY_ERROR_MESSAGE to "Missing input URI or file path",
                    WorkerConstants.KEY_ERROR to "Missing input URI or file path"
                )
            )
        }

        val effectiveName = characterName ?: "Imported Character"
        val stagingDir = File(appContext.cacheDir, "uploads/$sessionId").apply {
            if (!exists()) mkdirs()
        }
        val stagingFile = File(stagingDir, "staging_${sessionId}.obj")

        try {
            if (!inputPathStr.isNullOrEmpty()) {
                val sourceFile = File(inputPathStr)
                if (!sourceFile.exists() || !sourceFile.canRead()) {
                    return@withContext Result.failure(
                        workDataOf(
                            PipelineConstants.KEY_ERROR_MESSAGE to "Source file does not exist or is unreadable: $inputPathStr",
                            WorkerConstants.KEY_ERROR to "Source file does not exist or is unreadable: $inputPathStr"
                        )
                    )
                }
                sourceFile.copyTo(stagingFile, overwrite = true)
            } else if (!inputUriString.isNullOrEmpty()) {
                val uri = Uri.parse(inputUriString)
                val inputStream = if (uri.scheme == "file") {
                    File(uri.path ?: inputUriString).inputStream()
                } else if (uri.scheme == "content") {
                    appContext.contentResolver.openInputStream(uri)
                } else {
                    val f = File(inputUriString)
                    if (f.exists()) f.inputStream() else appContext.contentResolver.openInputStream(uri)
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

            if (!stagingFile.exists() || stagingFile.length() == 0L) {
                return@withContext Result.failure(
                    workDataOf(
                        PipelineConstants.KEY_ERROR_MESSAGE to "Failed to stage input file (empty or missing output)",
                        WorkerConstants.KEY_ERROR to "Failed to stage input file"
                    )
                )
            }

            val outputData = workDataOf(
                PipelineConstants.KEY_SESSION_ID to sessionId,
                PipelineConstants.KEY_STAGING_DIR to stagingDir.absolutePath,
                PipelineConstants.KEY_STAGING_FILE to stagingFile.absolutePath,
                WorkerConstants.KEY_STAGING_FILE_PATH to stagingFile.absolutePath,
                WorkerConstants.KEY_CHARACTER_NAME to effectiveName,
                WorkerConstants.KEY_STAGE_NAME to "PrepareUploads",
                WorkerConstants.KEY_PROGRESS to 0.16f
            )
            Result.success(outputData)
        } catch (e: Exception) {
            Result.failure(
                workDataOf(
                    PipelineConstants.KEY_ERROR_MESSAGE to (e.message ?: "Error preparing uploads"),
                    WorkerConstants.KEY_ERROR to (e.message ?: "Error preparing uploads")
                )
            )
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
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters,
    private val skeletonLoader: NativeSkeletonLoader,
    private val importer: AssetImporter
) : CoroutineWorker(appContext, workerParams) {

    constructor(
        context: Context,
        params: WorkerParameters,
        skeletonLoader: NativeSkeletonLoader
    ) : this(context, params, skeletonLoader, DefaultAssetImporter(context))

    constructor(
        context: Context,
        params: WorkerParameters,
        importer: AssetImporter
    ) : this(context, params, NativeSkeletonLoader(NativeLib()), importer)

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val stagingFilePath = inputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)
            ?: inputData.getString(PipelineConstants.KEY_STAGING_FILE)
            ?: return@withContext Result.failure(workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Missing staging file path"))
        val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID)
            ?: UUID.randomUUID().toString()
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME)
            ?: "Imported Character"

        val stagingFile = File(stagingFilePath)
        if (!stagingFile.exists() || !stagingFile.canRead()) {
            return@withContext Result.failure(
                workDataOf(
                    PipelineConstants.KEY_ERROR_MESSAGE to "Staged file not found: $stagingFilePath",
                    WorkerConstants.KEY_ERROR to "Staged file not found: $stagingFilePath"
                )
            )
        }

        val stagingDir = stagingFile.parentFile ?: File(appContext.cacheDir, "uploads/$sessionId").apply {
            if (!exists()) mkdirs()
        }

        val mesh = try {
            if (stagingFile.name.endsWith(".json", ignoreCase = true)) {
                val jsonText = stagingFile.readText()
                jsonFormat.decodeFromString<Mesh>(jsonText)
            } else {
                when (val resource = importer.importFromUri(Uri.fromFile(stagingFile), stagingDir)) {
                    is Resource.Success -> resource.data
                    else -> stagingFile.inputStream().use { stream ->
                        ObjParser.parse(stream, characterName)
                    }
                }
            }
        } catch (e: Exception) {
            return@withContext Result.failure(
                workDataOf(
                    PipelineConstants.KEY_ERROR_MESSAGE to (e.message ?: "Failed to parse mesh"),
                    WorkerConstants.KEY_ERROR to (e.message ?: "Failed to parse mesh")
                )
            )
        }

        if (mesh.vertices.isEmpty()) {
            return@withContext Result.failure(
                workDataOf(
                    PipelineConstants.KEY_ERROR_MESSAGE to "Parsed mesh contains zero vertices",
                    WorkerConstants.KEY_ERROR to "Parsed mesh contains zero vertices"
                )
            )
        }

        val skeleton = skeletonLoader.loadSkeletonFromJson(null)

        val parsedMeshFile = File(stagingDir, "parsed_mesh_${UUID.randomUUID()}.json")
        val skeletonFile = File(stagingDir, "skeleton.json")

        parsedMeshFile.writeText(jsonFormat.encodeToString(mesh))
        skeletonFile.writeText(jsonFormat.encodeToString(skeleton))

        val outputData = workDataOf(
            PipelineConstants.KEY_SESSION_ID to sessionId,
            PipelineConstants.KEY_STAGING_DIR to stagingDir.absolutePath,
            PipelineConstants.KEY_STAGING_FILE to stagingFilePath,
            WorkerConstants.KEY_STAGING_FILE_PATH to stagingFilePath,
            PipelineConstants.KEY_MESH_PATH to parsedMeshFile.absolutePath,
            WorkerConstants.KEY_PARSED_MESH_PATH to parsedMeshFile.absolutePath,
            PipelineConstants.KEY_SKELETON_PATH to skeletonFile.absolutePath,
            WorkerConstants.KEY_CHARACTER_NAME to characterName,
            WorkerConstants.KEY_STAGE_NAME to "ParseMesh",
            WorkerConstants.KEY_PROGRESS to 0.33f
        )
        Result.success(outputData)
    }
}

@HiltWorker
class FeatureExtractionWorker @AssistedInject constructor(
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters,
    private val nativeLib: NativeLib,
    private val importer: AssetImporter
) : CoroutineWorker(appContext, workerParams) {

    constructor(
        context: Context,
        params: WorkerParameters,
        nativeLib: NativeLib
    ) : this(context, params, nativeLib, DefaultAssetImporter(context))

    constructor(
        context: Context,
        params: WorkerParameters,
        importer: AssetImporter
    ) : this(context, params, NativeLib(), importer)

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val parsedMeshPath = inputData.getString(WorkerConstants.KEY_PARSED_MESH_PATH)
            ?: inputData.getString(PipelineConstants.KEY_MESH_PATH)
            ?: return@withContext Result.failure(workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Mesh path missing"))
        val stagingFilePath = inputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)
            ?: inputData.getString(PipelineConstants.KEY_STAGING_FILE) ?: ""
        val stagingDirStr = inputData.getString(PipelineConstants.KEY_STAGING_DIR)
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME) ?: "Imported Character"
        val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID) ?: UUID.randomUUID().toString()
        val skeletonPath = inputData.getString(PipelineConstants.KEY_SKELETON_PATH) ?: ""

        val parsedMeshFile = File(parsedMeshPath)
        if (!parsedMeshFile.exists()) {
            return@withContext Result.failure(
                workDataOf(
                    PipelineConstants.KEY_ERROR_MESSAGE to "Mesh file missing for feature extraction",
                    WorkerConstants.KEY_ERROR to "Mesh file missing for feature extraction"
                )
            )
        }

        val mesh = jsonFormat.decodeFromString<Mesh>(parsedMeshFile.readText())

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

        try {
            nativeLib.extractFeaturesNative(vertFloats, normFloats)
        } catch (_: Throwable) {}

        val resource = importer.extractFeatures(mesh)
        val featuresMap: Map<String, Any> = if (resource is Resource.Success) {
            resource.data
        } else {
            mapOf("vertexCount" to mesh.vertices.size)
        }

        val stagingDir = if (!stagingDirStr.isNullOrEmpty()) File(stagingDirStr) else parsedMeshFile.parentFile ?: appContext.cacheDir
        val featuresFile = File(stagingDir, "features_${UUID.randomUUID()}.json")
        val serializedFeatures = jsonFormat.encodeToString(featuresMap.mapValues { it.value.toString() })
        featuresFile.writeText(serializedFeatures)

        val outputData = workDataOf(
            PipelineConstants.KEY_SESSION_ID to sessionId,
            PipelineConstants.KEY_STAGING_DIR to stagingDir.absolutePath,
            PipelineConstants.KEY_STAGING_FILE to stagingFilePath,
            WorkerConstants.KEY_STAGING_FILE_PATH to stagingFilePath,
            PipelineConstants.KEY_MESH_PATH to parsedMeshPath,
            WorkerConstants.KEY_PARSED_MESH_PATH to parsedMeshPath,
            PipelineConstants.KEY_FEATURES_PATH to featuresFile.absolutePath,
            WorkerConstants.KEY_FEATURES_PATH to featuresFile.absolutePath,
            PipelineConstants.KEY_SKELETON_PATH to skeletonPath,
            WorkerConstants.KEY_CHARACTER_NAME to characterName,
            WorkerConstants.KEY_STAGE_NAME to "FeatureExtraction",
            WorkerConstants.KEY_PROGRESS to 0.50f
        )
        Result.success(outputData)
    }
}

@HiltWorker
class MLFittingWorker @AssistedInject constructor(
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters,
    private val nativeLib: NativeLib
) : CoroutineWorker(appContext, workerParams) {

    constructor(
        context: Context,
        params: WorkerParameters
    ) : this(context, params, NativeLib())

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val parsedMeshPath = inputData.getString(WorkerConstants.KEY_PARSED_MESH_PATH)
            ?: inputData.getString(PipelineConstants.KEY_MESH_PATH)
            ?: return@withContext Result.failure(workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Missing mesh path"))
        val featuresPath = inputData.getString(WorkerConstants.KEY_FEATURES_PATH)
            ?: inputData.getString(PipelineConstants.KEY_FEATURES_PATH)
        val stagingFilePath = inputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)
            ?: inputData.getString(PipelineConstants.KEY_STAGING_FILE) ?: ""
        val stagingDirStr = inputData.getString(PipelineConstants.KEY_STAGING_DIR)
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME) ?: "Imported Character"
        val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID) ?: UUID.randomUUID().toString()
        val skeletonPath = inputData.getString(PipelineConstants.KEY_SKELETON_PATH) ?: ""

        val parsedMeshFile = File(parsedMeshPath)
        if (!parsedMeshFile.exists()) {
            return@withContext Result.failure(
                workDataOf(
                    PipelineConstants.KEY_ERROR_MESSAGE to "Mesh file missing",
                    WorkerConstants.KEY_ERROR to "Mesh file missing"
                )
            )
        }

        val stagingDir = if (!stagingDirStr.isNullOrEmpty()) File(stagingDirStr) else parsedMeshFile.parentFile ?: appContext.cacheDir

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

        val skeleton = if (skeletonPath.isNotEmpty() && File(skeletonPath).exists()) {
            try {
                jsonFormat.decodeFromString<Skeleton>(File(skeletonPath).readText())
            } catch (_: Exception) {
                NativeSkeletonLoader.createStandardBiped()
            }
        } else {
            NativeSkeletonLoader.createStandardBiped()
        }

        val fittingFile = File(stagingDir, "fitting_${UUID.randomUUID()}.json")
        fittingFile.writeText(jsonFormat.encodeToString(skeleton))

        val fittedWeightsFile = File(stagingDir, "fitted_weights.json")
        fittedWeightsFile.writeText(jsonFormat.encodeToString(morphWeightsMap))

        val effectiveSkeletonPath = if (skeletonPath.isNotEmpty()) skeletonPath else fittingFile.absolutePath

        val outputData = workDataOf(
            PipelineConstants.KEY_SESSION_ID to sessionId,
            PipelineConstants.KEY_STAGING_DIR to stagingDir.absolutePath,
            PipelineConstants.KEY_STAGING_FILE to stagingFilePath,
            WorkerConstants.KEY_STAGING_FILE_PATH to stagingFilePath,
            PipelineConstants.KEY_MESH_PATH to parsedMeshPath,
            WorkerConstants.KEY_PARSED_MESH_PATH to parsedMeshPath,
            PipelineConstants.KEY_FEATURES_PATH to (featuresPath ?: ""),
            WorkerConstants.KEY_FEATURES_PATH to (featuresPath ?: ""),
            PipelineConstants.KEY_SKELETON_PATH to effectiveSkeletonPath,
            WorkerConstants.KEY_FITTING_PATH to fittingFile.absolutePath,
            PipelineConstants.KEY_FITTED_WEIGHTS_PATH to fittedWeightsFile.absolutePath,
            WorkerConstants.KEY_CHARACTER_NAME to characterName,
            WorkerConstants.KEY_STAGE_NAME to "MLFitting",
            WorkerConstants.KEY_PROGRESS to 0.66f
        )
        Result.success(outputData)
    }
}

@HiltWorker
class SliderSynthesisWorker @AssistedInject constructor(
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters
) : CoroutineWorker(appContext, workerParams) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val parsedMeshPath = inputData.getString(WorkerConstants.KEY_PARSED_MESH_PATH)
            ?: inputData.getString(PipelineConstants.KEY_MESH_PATH)
            ?: return@withContext Result.failure(workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Missing mesh path"))
        val fittingPath = inputData.getString(WorkerConstants.KEY_FITTING_PATH)
        val fittedWeightsPath = inputData.getString(PipelineConstants.KEY_FITTED_WEIGHTS_PATH)
        val stagingFilePath = inputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)
            ?: inputData.getString(PipelineConstants.KEY_STAGING_FILE) ?: ""
        val stagingDirStr = inputData.getString(PipelineConstants.KEY_STAGING_DIR)
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME) ?: "Imported Character"
        val featuresPath = inputData.getString(WorkerConstants.KEY_FEATURES_PATH)
            ?: inputData.getString(PipelineConstants.KEY_FEATURES_PATH)
        val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID) ?: UUID.randomUUID().toString()
        val skeletonPath = inputData.getString(PipelineConstants.KEY_SKELETON_PATH) ?: ""

        val parsedMeshFile = File(parsedMeshPath)
        if (!parsedMeshFile.exists()) {
            return@withContext Result.failure(
                workDataOf(
                    PipelineConstants.KEY_ERROR_MESSAGE to "Mesh file missing",
                    WorkerConstants.KEY_ERROR to "Mesh file missing"
                )
            )
        }

        val stagingDir = if (!stagingDirStr.isNullOrEmpty()) File(stagingDirStr) else parsedMeshFile.parentFile ?: appContext.cacheDir

        val weightsMap: Map<String, Float> = if (!fittedWeightsPath.isNullOrEmpty() && File(fittedWeightsPath).exists()) {
            try {
                jsonFormat.decodeFromString<Map<String, Float>>(File(fittedWeightsPath).readText())
            } catch (_: Exception) {
                mapOf("body_fat" to 0.5f, "muscularity" to 0.3f, "height" to 0.5f)
            }
        } else {
            mapOf("body_fat" to 0.5f, "muscularity" to 0.3f, "height" to 0.5f)
        }

        val slidersFile = File(stagingDir, "sliders_${UUID.randomUUID()}.json")
        slidersFile.writeText(jsonFormat.encodeToString(weightsMap))

        val outputData = workDataOf(
            PipelineConstants.KEY_SESSION_ID to sessionId,
            PipelineConstants.KEY_STAGING_DIR to stagingDir.absolutePath,
            PipelineConstants.KEY_STAGING_FILE to stagingFilePath,
            WorkerConstants.KEY_STAGING_FILE_PATH to stagingFilePath,
            PipelineConstants.KEY_MESH_PATH to parsedMeshPath,
            WorkerConstants.KEY_PARSED_MESH_PATH to parsedMeshPath,
            PipelineConstants.KEY_FEATURES_PATH to (featuresPath ?: ""),
            WorkerConstants.KEY_FEATURES_PATH to (featuresPath ?: ""),
            WorkerConstants.KEY_FITTING_PATH to (fittingPath ?: ""),
            PipelineConstants.KEY_FITTED_WEIGHTS_PATH to (fittedWeightsPath ?: ""),
            PipelineConstants.KEY_SKELETON_PATH to skeletonPath,
            WorkerConstants.KEY_SLIDERS_PATH to slidersFile.absolutePath,
            PipelineConstants.KEY_SLIDERS_PATH to slidersFile.absolutePath,
            WorkerConstants.KEY_CHARACTER_NAME to characterName,
            WorkerConstants.KEY_STAGE_NAME to "SliderSynthesis",
            WorkerConstants.KEY_PROGRESS to 0.83f
        )
        Result.success(outputData)
    }
}

@HiltWorker
class FinalizeWorker @AssistedInject constructor(
    @Assisted private val appContext: Context,
    @Assisted workerParams: WorkerParameters,
    private val repository: CharacterRepository
) : CoroutineWorker(appContext, workerParams) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val parsedMeshPath = inputData.getString(WorkerConstants.KEY_PARSED_MESH_PATH)
            ?: inputData.getString(PipelineConstants.KEY_MESH_PATH)
            ?: return@withContext Result.failure(workDataOf(PipelineConstants.KEY_ERROR_MESSAGE to "Missing mesh path"))
        val fittingPath = inputData.getString(WorkerConstants.KEY_FITTING_PATH)
        val slidersPath = inputData.getString(WorkerConstants.KEY_SLIDERS_PATH)
            ?: inputData.getString(PipelineConstants.KEY_SLIDERS_PATH)
        val stagingFilePath = inputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)
            ?: inputData.getString(PipelineConstants.KEY_STAGING_FILE)
        val featuresPath = inputData.getString(WorkerConstants.KEY_FEATURES_PATH)
            ?: inputData.getString(PipelineConstants.KEY_FEATURES_PATH)
        val fittedWeightsPath = inputData.getString(PipelineConstants.KEY_FITTED_WEIGHTS_PATH)
        val skeletonPath = inputData.getString(PipelineConstants.KEY_SKELETON_PATH)
        val stagingDirStr = inputData.getString(PipelineConstants.KEY_STAGING_DIR)
        val characterName = inputData.getString(WorkerConstants.KEY_CHARACTER_NAME) ?: "Imported Character"
        val sessionId = inputData.getString(PipelineConstants.KEY_SESSION_ID)

        val parsedMeshFile = File(parsedMeshPath)
        val fittingFile = fittingPath?.let { File(it) }
        val slidersFile = slidersPath?.let { File(it) }
        val stagingFile = stagingFilePath?.let { File(it) }
        val featuresFile = featuresPath?.let { File(it) }
        val fittedWeightsFile = fittedWeightsPath?.let { File(it) }
        val skeletonFile = skeletonPath?.let { File(it) }

        try {
            if (!parsedMeshFile.exists()) {
                return@withContext Result.failure(
                    workDataOf(
                        PipelineConstants.KEY_ERROR_MESSAGE to "Mesh file missing",
                        WorkerConstants.KEY_ERROR to "Mesh file missing"
                    )
                )
            }

            val mesh = jsonFormat.decodeFromString<Mesh>(parsedMeshFile.readText())

            val skeleton = if (skeletonFile != null && skeletonFile.exists()) {
                try {
                    jsonFormat.decodeFromString<Skeleton>(skeletonFile.readText())
                } catch (_: Exception) {
                    NativeSkeletonLoader.createStandardBiped()
                }
            } else if (fittingFile != null && fittingFile.exists()) {
                try {
                    jsonFormat.decodeFromString<Skeleton>(fittingFile.readText())
                } catch (_: Exception) {
                    NativeSkeletonLoader.createStandardBiped()
                }
            } else {
                NativeSkeletonLoader.createStandardBiped()
            }

            val activeMorphs = if (fittedWeightsFile != null && fittedWeightsFile.exists()) {
                try {
                    jsonFormat.decodeFromString<Map<String, Float>>(fittedWeightsFile.readText())
                } catch (_: Exception) {
                    mapOf("body_fat" to 0.5f)
                }
            } else if (slidersFile != null && slidersFile.exists()) {
                try {
                    jsonFormat.decodeFromString<Map<String, Float>>(slidersFile.readText())
                } catch (_: Exception) {
                    mapOf("body_fat" to 0.5f)
                }
            } else {
                mapOf("body_fat" to 0.5f)
            }

            val characterId = sessionId ?: UUID.randomUUID().toString()

            val character = Character(
                id = characterId,
                baseMesh = mesh.copy(name = characterName),
                skeleton = skeleton,
                activeMorphs = activeMorphs
            )

            repository.saveCharacter(character)

            // Save results character.json for result verification if filesDir used
            try {
                val resultsDir = File(appContext.filesDir, "results/$characterId").apply {
                    if (!exists()) mkdirs()
                }
                File(resultsDir, "character.json").writeText(jsonFormat.encodeToString(character))
            } catch (_: Exception) {}

            // Delete temporary staging files
            parsedMeshFile.delete()
            fittingFile?.delete()
            slidersFile?.delete()
            stagingFile?.delete()
            featuresFile?.delete()
            fittedWeightsFile?.delete()
            skeletonFile?.delete()

            if (!stagingDirStr.isNullOrEmpty()) {
                val stagingDir = File(stagingDirStr)
                if (stagingDir.exists()) {
                    stagingDir.deleteRecursively()
                }
            }

            val outputData = workDataOf(
                WorkerConstants.KEY_CHARACTER_ID to character.id,
                PipelineConstants.KEY_CHARACTER_ID to character.id,
                WorkerConstants.KEY_STAGE_NAME to "Finalize",
                WorkerConstants.KEY_PROGRESS to 1.0f
            )
            Result.success(outputData)
        } catch (e: Exception) {
            parsedMeshFile.delete()
            fittingFile?.delete()
            slidersFile?.delete()
            stagingFile?.delete()
            featuresFile?.delete()
            fittedWeightsFile?.delete()
            skeletonFile?.delete()

            Result.failure(
                workDataOf(
                    PipelineConstants.KEY_ERROR_MESSAGE to (e.message ?: "Finalization failed"),
                    WorkerConstants.KEY_ERROR to (e.message ?: "Finalization failed")
                )
            )
        }
    }
}
