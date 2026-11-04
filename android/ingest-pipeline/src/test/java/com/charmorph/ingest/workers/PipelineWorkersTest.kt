package com.charmorph.ingest.workers

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.work.ListenableWorker
import androidx.work.ListenableWorker.Result
import androidx.work.WorkerFactory
import androidx.work.WorkerParameters
import androidx.work.testing.TestListenableWorkerBuilder
import androidx.work.workDataOf
import com.charmorph.core.model.Character
import com.charmorph.ingest.DefaultAssetImporter
import com.charmorph.storage.CharacterRepository
import com.charmorph.storage.dao.CharacterDao
import com.charmorph.storage.entity.CharacterGeometryEntity
import com.charmorph.storage.entity.CharacterMetadataEntity
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import java.io.File

class FakePipelineCharacterDao : CharacterDao {
    override fun getAllMetadata(): Flow<List<CharacterMetadataEntity>> = flowOf(emptyList())
    override suspend fun getMetadataById(id: String): CharacterMetadataEntity? = null
    override suspend fun getGeometryById(id: String): CharacterGeometryEntity? = null
    override suspend fun insertMetadata(metadata: CharacterMetadataEntity) {}
    override suspend fun insertGeometry(geometry: CharacterGeometryEntity) {}
    override suspend fun updateMorphWeights(id: String, weights: Map<String, Float>, lastModified: Long) {}
    override suspend fun deleteCharacter(id: String) {}
}

class FakePipelineRepository : CharacterRepository(
    characterDao = FakePipelineCharacterDao()
) {
    var savedCharacter: Character? = null

    override suspend fun saveCharacter(character: Character) {
        savedCharacter = character
    }
}

@RunWith(AndroidJUnit4::class)
@Config(manifest = Config.NONE)
class PipelineWorkersTest {

    private lateinit var context: Context
    private lateinit var importer: DefaultAssetImporter
    private lateinit var repository: FakePipelineRepository

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        importer = DefaultAssetImporter(context)
        repository = FakePipelineRepository()
    }

    @Test
    fun testPrepareUploadsWorkerCreatesStagingFile() = runTest {
        val worker = TestListenableWorkerBuilder<PrepareUploadsWorker>(context)
            .setInputData(workDataOf(WorkerConstants.KEY_CHARACTER_NAME to "Test Hero"))
            .build()

        val result = worker.doWork()
        assertTrue(result is Result.Success)

        val outputData = (result as Result.Success).outputData
        val stagingPath = outputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)
        assertNotNull(stagingPath)
        val stagingFile = File(stagingPath!!)
        assertTrue(stagingFile.exists())
        assertTrue(stagingFile.length() > 0)

        stagingFile.delete()
    }

    @Test
    fun testFullPipelineChainWithCleanup() = runTest {
        val workerFactory = object : WorkerFactory() {
            override fun createWorker(
                appContext: Context,
                workerClassName: String,
                workerParameters: WorkerParameters
            ): ListenableWorker? {
                return when (workerClassName) {
                    PrepareUploadsWorker::class.java.name -> PrepareUploadsWorker(appContext, workerParameters)
                    ParseMeshWorker::class.java.name -> ParseMeshWorker(appContext, workerParameters, importer)
                    FeatureExtractionWorker::class.java.name -> FeatureExtractionWorker(appContext, workerParameters, importer)
                    MLFittingWorker::class.java.name -> MLFittingWorker(appContext, workerParameters)
                    SliderSynthesisWorker::class.java.name -> SliderSynthesisWorker(appContext, workerParameters)
                    FinalizeWorker::class.java.name -> FinalizeWorker(appContext, workerParameters, repository)
                    else -> null
                }
            }
        }

        // 1. PrepareUploads
        val prepareWorker = TestListenableWorkerBuilder<PrepareUploadsWorker>(context)
            .setWorkerFactory(workerFactory)
            .setInputData(workDataOf(WorkerConstants.KEY_CHARACTER_NAME to "Pipeline Hero"))
            .build()
        val prepareResult = prepareWorker.doWork() as Result.Success
        val stagingPath = prepareResult.outputData.getString(WorkerConstants.KEY_STAGING_FILE_PATH)!!

        // 2. ParseMesh
        val parseWorker = TestListenableWorkerBuilder<ParseMeshWorker>(context)
            .setWorkerFactory(workerFactory)
            .setInputData(prepareResult.outputData)
            .build()
        val parseResult = parseWorker.doWork() as Result.Success
        val parsedMeshPath = parseResult.outputData.getString(WorkerConstants.KEY_PARSED_MESH_PATH)!!

        // 3. FeatureExtraction
        val featureWorker = TestListenableWorkerBuilder<FeatureExtractionWorker>(context)
            .setWorkerFactory(workerFactory)
            .setInputData(parseResult.outputData)
            .build()
        val featureResult = featureWorker.doWork() as Result.Success
        val featuresPath = featureResult.outputData.getString(WorkerConstants.KEY_FEATURES_PATH)!!

        // 4. MLFitting
        val mlWorker = TestListenableWorkerBuilder<MLFittingWorker>(context)
            .setWorkerFactory(workerFactory)
            .setInputData(featureResult.outputData)
            .build()
        val mlResult = mlWorker.doWork() as Result.Success
        val fittingPath = mlResult.outputData.getString(WorkerConstants.KEY_FITTING_PATH)!!

        // 5. SliderSynthesis
        val sliderWorker = TestListenableWorkerBuilder<SliderSynthesisWorker>(context)
            .setWorkerFactory(workerFactory)
            .setInputData(mlResult.outputData)
            .build()
        val sliderResult = sliderWorker.doWork() as Result.Success
        val slidersPath = sliderResult.outputData.getString(WorkerConstants.KEY_SLIDERS_PATH)!!

        // Verify intermediate files exist before finalize
        assertTrue(File(stagingPath).exists())
        assertTrue(File(parsedMeshPath).exists())
        assertTrue(File(featuresPath).exists())
        assertTrue(File(fittingPath).exists())
        assertTrue(File(slidersPath).exists())

        // 6. Finalize
        val finalizeWorker = TestListenableWorkerBuilder<FinalizeWorker>(context)
            .setWorkerFactory(workerFactory)
            .setInputData(sliderResult.outputData)
            .build()
        val finalizeResult = finalizeWorker.doWork() as Result.Success

        // Verify character persisted to repository
        val character = repository.savedCharacter
        assertNotNull(character)
        assertEquals("Pipeline Hero", character?.baseMesh?.name)
        assertTrue(character?.skeleton?.bones?.isNotEmpty() == true)

        // Verify Constraint 3: Temporary staging files deleted
        assertFalse(File(stagingPath).exists())
        assertFalse(File(parsedMeshPath).exists())
        assertFalse(File(featuresPath).exists())
        assertFalse(File(fittingPath).exists())
        assertFalse(File(slidersPath).exists())
    }
}
