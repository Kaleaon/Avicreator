package com.charmorph.app.ui

import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Skeleton
import com.charmorph.core.model.Vector2
import com.charmorph.core.model.Vector3
import com.charmorph.storage.AppDatabase
import com.charmorph.storage.CharacterRepository
import com.charmorph.storage.entity.CharacterGeometryEntity
import com.charmorph.storage.entity.CharacterMetadataEntity
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import kotlin.system.measureTimeMillis

@RunWith(AndroidJUnit4::class)
@Config(manifest = Config.NONE)
class CatalogBenchmarkTest {

    private lateinit var database: AppDatabase
    private lateinit var repository: CharacterRepository

    @Before
    fun setUp() {
        database = Room.inMemoryDatabaseBuilder(
            ApplicationProvider.getApplicationContext(),
            AppDatabase::class.java
        ).allowMainThreadQueries().build()

        repository = CharacterRepository(database.characterDao())
    }

    @After
    fun tearDown() {
        database.close()
    }

    @Test
    fun benchmarkCatalogLoadWithNormalizedMetadata() = runBlocking {
        val dao = database.characterDao()

        // Populate database with 50 characters containing heavy mesh geometry payloads (10,000 vertices each)
        val heavyVertices = List(10000) { Vector3(it.toFloat(), it.toFloat(), it.toFloat()) }
        val heavyNormals = List(10000) { Vector3(0f, 1f, 0f) }
        val heavyUvs = List(10000) { Vector2(0.5f, 0.5f) }
        val heavyIndices = List(30000) { it % 10000 }

        val baseTime = System.currentTimeMillis()

        for (i in 1..50) {
            val charId = "char-$i"
            val metadata = CharacterMetadataEntity(
                id = charId,
                name = "Character $i",
                thumbnailPath = "/thumbs/$i.png",
                lastModified = baseTime - i * 100,
                morphWeights = mapOf("body_fat" to 0.5f, "body_muscle" to 0.2f)
            )

            val heavyMesh = Mesh(
                id = "mesh-$i",
                name = "Heavy Mesh $i",
                vertices = heavyVertices,
                normals = heavyNormals,
                uvs = heavyUvs,
                indices = heavyIndices
            )

            val geometry = CharacterGeometryEntity(
                characterId = charId,
                meshData = heavyMesh,
                skeletonData = Skeleton(emptyList())
            )

            dao.insertCharacter(metadata, geometry)
        }

        // Warm-up query execution to eliminate JVM/Robolectric initial class loading overhead
        repository.allCharacterMetadata.first()

        // Benchmark catalog query load time
        val durationMs = measureTimeMillis {
            val catalogItems = repository.allCharacterMetadata.first()
            assertEquals(50, catalogItems.size)
            assertEquals("Character 1", catalogItems[0].name)
        }

        println("Catalog benchmark duration for 50 heavy characters: $durationMs ms")

        // Acceptance Criterion 1 & Target metric: Catalog query loads under 50ms without geometry deserialization
        assertTrue("Catalog query must take < 50ms (actual: $durationMs ms)", durationMs < 50)
    }
}
