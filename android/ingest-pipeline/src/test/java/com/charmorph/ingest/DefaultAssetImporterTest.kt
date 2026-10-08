package com.charmorph.ingest

import android.content.Context
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.MeshGroup
import com.charmorph.core.model.Resource
import com.charmorph.core.model.Vector3
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import java.io.File

@RunWith(AndroidJUnit4::class)
@Config(manifest = Config.NONE)
class DefaultAssetImporterTest {

    private lateinit var context: Context
    private lateinit var importer: DefaultAssetImporter

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        importer = DefaultAssetImporter(context)
    }

    @Test
    fun testImportFromUriWithObjFile() = runTest {
        val testObjContent = """
            # Simple Cube OBJ
            v -1.0 -1.0 1.0
            v 1.0 -1.0 1.0
            v 1.0 1.0 1.0
            v -1.0 1.0 1.0
            g cube_group
            f 1 2 3
            f 1 3 4
        """.trimIndent()

        val tempFile = File.createTempFile("test_mesh", ".obj", context.cacheDir)
        tempFile.writeText(testObjContent)

        val uri = Uri.fromFile(tempFile)
        val result = importer.importFromUri(uri, context.cacheDir)

        assertTrue(result is Resource.Success)
        val mesh = (result as Resource.Success).data
        assertEquals(4, mesh.vertices.size)
        assertEquals(6, mesh.indices.size)

        tempFile.delete()
    }

    @Test
    fun testExtractFeaturesFromMesh() = runTest {
        val mesh = Mesh(
            id = "test-id",
            name = "Test Mesh",
            vertices = listOf(
                Vector3(-1f, -2f, -3f),
                Vector3(1f, 2f, 3f)
            ),
            normals = emptyList(),
            uvs = emptyList(),
            indices = listOf(0, 1, 0),
            groups = listOf(MeshGroup("body", listOf(0, 1, 0), 0, listOf("anatomical")))
        )

        val result = importer.extractFeatures(mesh)
        assertTrue(result is Resource.Success)

        val features = (result as Resource.Success).data
        assertEquals(2, features["vertexCount"])
        assertEquals(1, features["faceCount"])
        assertEquals(2f, features["boundsWidth"])
        assertEquals(4f, features["boundsHeight"])
        assertEquals(6f, features["boundsDepth"])
    }
}
