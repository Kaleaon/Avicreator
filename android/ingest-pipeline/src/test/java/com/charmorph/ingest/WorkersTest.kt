package com.charmorph.ingest

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.work.ListenableWorker.Result
import androidx.work.testing.TestListenableWorkerBuilder
import androidx.work.workDataOf
import com.charmorph.core.model.CharacterMetadata
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Vector2
import com.charmorph.core.model.Vector3
import com.charmorph.ingest.workers.FeatureExtractionWorker
import com.charmorph.ingest.workers.FinalizeWorker
import com.charmorph.ingest.workers.MLFittingWorker
import com.charmorph.ingest.workers.ParseMeshWorker
import com.charmorph.ingest.workers.PipelineConstants
import com.charmorph.ingest.workers.PrepareUploadsWorker
import com.charmorph.ingest.workers.SliderSynthesisWorker
import com.charmorph.nativebridge.NativeLib
import com.charmorph.storage.CharacterRepository
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.encodeToString
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.UUID

@RunWith(AndroidJUnit4::class)
class WorkersTest {

    private lateinit var context: Context
    private lateinit var fakeRepository: FakeCharacterRepository
    private val nativeLib = NativeLib()
    private val skeletonLoader = NativeSkeletonLoader(nativeLib)

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        fakeRepository = FakeCharacterRepository()
    }

    @Test
    fun testPrepareUploadsWorkerWithMissingInputFailsGracefully() = runBlocking {
        val worker = TestListenableWorkerBuilder<PrepareUploadsWorker>(context)
            .setInputData(workDataOf(PipelineConstants.KEY_SESSION_ID to "test_session"))
            .build()

        val result = worker.doWork()
        assertTrue(result is Result.Failure)
    }

    @Test
    fun testPrepareUploadsWorkerWithValidFileSucceeds() = runBlocking {
        val testFile = File(context.cacheDir, "test_input.obj").apply {
            writeText("v 0.0 0.0 0.0\nv 1.0 0.0 0.0\nv 0.0 1.0 0.0\nf 1 2 3\n")
        }

        val sessionId = "session_123"
        val worker = TestListenableWorkerBuilder<PrepareUploadsWorker>(context)
            .setInputData(
                workDataOf(
                    PipelineConstants.KEY_SESSION_ID to sessionId,
                    PipelineConstants.KEY_INPUT_PATH to testFile.absolutePath
                )
            )
            .build()

        val result = worker.doWork()
        assertTrue(result is Result.Success)

        val outputData = (result as Result.Success).outputData
        val stagingDirStr = outputData.getString(PipelineConstants.KEY_STAGING_DIR)
        assertNotNull(stagingDirStr)
        assertTrue(File(stagingDirStr!!).exists())

        // Verify Data payload size is strictly under 10 KB (10240 bytes)
        assertTrue(outputData.toByteArray().size < 2048)
    }

    @Test
    fun testFullSequentialPipelineExecution() = runBlocking {
        val sessionId = UUID.randomUUID().toString()

        // 1. Prepare
        val testObjFile = File(context.cacheDir, "sample.obj").apply {
            writeText("v 0.0 0.0 0.0\nv 1.0 0.0 0.0\nv 0.0 1.0 0.0\nf 1 2 3\n")
        }

        val prepareWorker = TestListenableWorkerBuilder<PrepareUploadsWorker>(context)
            .setInputData(
                workDataOf(
                    PipelineConstants.KEY_SESSION_ID to sessionId,
                    PipelineConstants.KEY_INPUT_PATH to testObjFile.absolutePath
                )
            )
            .build()

        val prepareResult = prepareWorker.doWork()
        assertTrue("PrepareWorker failed: ${(prepareResult as? Result.Failure)?.outputData?.getString(PipelineConstants.KEY_ERROR_MESSAGE)}", prepareResult is Result.Success)
        val prepareData = (prepareResult as Result.Success).outputData

        // 2. Parse
        val parseWorker = TestListenableWorkerBuilder<ParseMeshWorker>(context)
            .setWorkerFactory(SingleWorkerFactory { ParseMeshWorker(context, it, skeletonLoader) })
            .setInputData(prepareData)
            .build()

        val parseResult = parseWorker.doWork()
        assertTrue("ParseWorker failed: ${(parseResult as? Result.Failure)?.outputData?.getString(PipelineConstants.KEY_ERROR_MESSAGE)}", parseResult is Result.Success)
        val parseData = (parseResult as Result.Success).outputData

        // 3. Feature Extraction
        val featureWorker = TestListenableWorkerBuilder<FeatureExtractionWorker>(context)
            .setWorkerFactory(SingleWorkerFactory { FeatureExtractionWorker(context, it, nativeLib) })
            .setInputData(parseData)
            .build()

        val featureResult = featureWorker.doWork()
        assertTrue("FeatureWorker failed: ${(featureResult as? Result.Failure)?.outputData?.getString(PipelineConstants.KEY_ERROR_MESSAGE)}", featureResult is Result.Success)
        val featureData = (featureResult as Result.Success).outputData

        // 4. ML Fitting
        val mlWorker = TestListenableWorkerBuilder<MLFittingWorker>(context)
            .setWorkerFactory(SingleWorkerFactory { MLFittingWorker(context, it, nativeLib) })
            .setInputData(featureData)
            .build()

        val mlResult = mlWorker.doWork()
        assertTrue("MLWorker failed: ${(mlResult as? Result.Failure)?.outputData?.getString(PipelineConstants.KEY_ERROR_MESSAGE)}", mlResult is Result.Success)
        val mlData = (mlResult as Result.Success).outputData

        // 5. Slider Synthesis
        val sliderWorker = TestListenableWorkerBuilder<SliderSynthesisWorker>(context)
            .setInputData(mlData)
            .build()

        val sliderResult = sliderWorker.doWork()
        assertTrue("SliderWorker failed: ${(sliderResult as? Result.Failure)?.outputData?.getString(PipelineConstants.KEY_ERROR_MESSAGE)}", sliderResult is Result.Success)
        val sliderData = (sliderResult as Result.Success).outputData

        // 6. Finalize
        val finalizeWorker = TestListenableWorkerBuilder<FinalizeWorker>(context)
            .setWorkerFactory(SingleWorkerFactory { FinalizeWorker(context, it, fakeRepository) })
            .setInputData(sliderData)
            .build()

        val finalizeResult = finalizeWorker.doWork()
        assertTrue("FinalizeWorker failed: ${(finalizeResult as? Result.Failure)?.outputData?.getString(PipelineConstants.KEY_ERROR_MESSAGE)}", finalizeResult is Result.Success)
        val finalizeData = (finalizeResult as Result.Success).outputData
        val finalCharacterId = finalizeData.getString(PipelineConstants.KEY_CHARACTER_ID)

        assertEquals(sessionId, finalCharacterId)
        assertNotNull(fakeRepository.savedCharacter)
        assertEquals(sessionId, fakeRepository.savedCharacter?.id)
    }
}

// Simple Fake Repository for testing
class FakeCharacterRepository : CharacterRepository(
    object : com.charmorph.storage.dao.CharacterDao {
        override fun getAllMetadata(): Flow<List<com.charmorph.storage.entity.CharacterMetadataEntity>> = flowOf(emptyList())
        override suspend fun getMetadataById(id: String): com.charmorph.storage.entity.CharacterMetadataEntity? = null
        override suspend fun getGeometryById(id: String): com.charmorph.storage.entity.CharacterGeometryEntity? = null
        override suspend fun insertMetadata(metadata: com.charmorph.storage.entity.CharacterMetadataEntity) {}
        override suspend fun insertGeometry(geometry: com.charmorph.storage.entity.CharacterGeometryEntity) {}
        override suspend fun updateMorphWeights(id: String, weights: Map<String, Float>, timestamp: Long) {}
        override suspend fun deleteCharacter(id: String) {}
    }
) {
    var savedCharacter: com.charmorph.core.model.Character? = null

    override suspend fun saveCharacter(character: com.charmorph.core.model.Character) {
        savedCharacter = character
    }
}

// Simple Worker Factory helper for Robolectric testing
class SingleWorkerFactory(
    private val creator: (androidx.work.WorkerParameters) -> androidx.work.ListenableWorker
) : androidx.work.WorkerFactory() {
    override fun createWorker(
        appContext: Context,
        workerClassName: String,
        workerParameters: androidx.work.WorkerParameters
    ): androidx.work.ListenableWorker {
        return creator(workerParameters)
    }
}
