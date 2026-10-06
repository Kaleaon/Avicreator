package com.charmorph.app.ui

import androidx.lifecycle.SavedStateHandle
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.charmorph.core.model.Character
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Skeleton
import com.charmorph.storage.CharacterRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

class FakeCharacterRepository : CharacterRepository(
    characterDao = FakeCharacterDao()
) {
    var updateCount = 0
    var lastUpdatedWeights: Map<String, Float>? = null

    val storedCharacter = Character(
        id = "test-char",
        baseMesh = Mesh("m1", "BaseMesh", emptyList(), emptyList(), emptyList(), emptyList()),
        skeleton = Skeleton(emptyList()),
        activeMorphs = mapOf("body_fat" to 0.0f)
    )

    override suspend fun getCharacter(id: String): Character? {
        return storedCharacter
    }

    override suspend fun updateMorphWeights(id: String, weights: Map<String, Float>) {
        updateCount++
        lastUpdatedWeights = weights
    }
}

class FakeCharacterDao : com.charmorph.storage.dao.CharacterDao {
    override fun getAllMetadata() = kotlinx.coroutines.flow.flowOf(emptyList<com.charmorph.storage.entity.CharacterMetadataEntity>())
    override suspend fun getMetadataById(id: String) = null
    override suspend fun getGeometryById(id: String) = null
    override suspend fun insertMetadata(metadata: com.charmorph.storage.entity.CharacterMetadataEntity) {}
    override suspend fun insertGeometry(geometry: com.charmorph.storage.entity.CharacterGeometryEntity) {}
    override suspend fun updateMorphWeights(id: String, weights: Map<String, Float>, lastModified: Long) {}
    override suspend fun deleteCharacter(id: String) {}
}

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(AndroidJUnit4::class)
@Config(manifest = Config.NONE)
class EditorViewModelTest {

    private val testDispatcher = StandardTestDispatcher()
    private val testScope = TestScope(testDispatcher)

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
    }

    @Test
    fun testImmediateUiUpdateAndDebouncedDatabaseWrite() = testScope.runTest {
        val repo = FakeCharacterRepository()
        val savedStateHandle = SavedStateHandle(mapOf("characterId" to "test-char"))
        val viewModel = EditorViewModel(repo, savedStateHandle)

        // Advance dispatcher to complete loadCharacter
        testScheduler.advanceUntilIdle()

        // Initial state
        assertEquals(0, repo.updateCount)

        // Simulate fast 60Hz slider gestures (60 updates over 1 second, every 16ms)
        val startTime = System.currentTimeMillis()
        for (i in 1..60) {
            val weightValue = i / 60f
            viewModel.updateMorph("body_fat", weightValue)

            // UI state updates synchronously in memory immediately
            val currentMorph = viewModel.uiState.value.morphs.find { it.name == "body_fat" }
            assertEquals(weightValue, currentMorph?.value ?: 0f, 0.001f)

            advanceTimeBy(16L) // 16ms between frames
        }

        // During continuous dragging (every 16ms < 300ms debounce interval),
        // zero database writes should have executed during the gesture frames
        assertEquals(0, repo.updateCount)

        // Advance time by 300ms after the gesture stops
        advanceTimeBy(300L)

        // Exactly 1 debounced write should have occurred, persisting final weight
        assertEquals(1, repo.updateCount)
        assertEquals(1.0f, repo.lastUpdatedWeights?.get("body_fat") ?: 0f, 0.001f)
    }

    @Test
    fun testImmediateFlushOnNavigateAway() = testScope.runTest {
        val repo = FakeCharacterRepository()
        val savedStateHandle = SavedStateHandle(mapOf("characterId" to "test-char"))
        val viewModel = EditorViewModel(repo, savedStateHandle)

        testScheduler.advanceUntilIdle()

        // User updates slider
        viewModel.updateMorph("body_fat", 0.75f)

        // UI state updated synchronously
        assertEquals(0.75f, viewModel.uiState.value.morphs.find { it.name == "body_fat" }?.value)

        // DB write has not happened yet (0ms < 300ms)
        assertEquals(0, repo.updateCount)

        // User immediately navigates away (triggers flushPendingWrites)
        viewModel.flushPendingWrites()

        // Database write executed immediately without waiting for 300ms debounce
        assertEquals(1, repo.updateCount)
        assertEquals(0.75f, repo.lastUpdatedWeights?.get("body_fat"))
    }
}
