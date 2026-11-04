package com.charmorph.feature.photoimport

import android.graphics.Bitmap
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.MorphTarget
import com.charmorph.core.model.Resource
import com.charmorph.core.model.Vector3
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config

@RunWith(AndroidJUnit4::class)
@Config(manifest = Config.NONE)
class PhotoToCharacterSolverTest {

    private val baseMesh = Mesh(
        id = "test_head",
        name = "TestHead",
        vertices = emptyList(),
        normals = emptyList(),
        uvs = emptyList(),
        indices = emptyList()
    )

    private val mockBitmap = Bitmap.createBitmap(100, 100, Bitmap.Config.ARGB_8888)

    @Test
    fun testSolveFromImageWithValidLandmarks() = runTest {
        val landmarks1 = listOf(
            Pair(20f, 30f),
            Pair(80f, 30f),
            Pair(50f, 50f),
            Pair(30f, 70f),
            Pair(70f, 70f)
        )

        val solver1 = PhotoToCharacterSolver(landmarkDetector = { landmarks1 })
        val result1 = solver1.solveFromImage(mockBitmap, baseMesh)

        assertTrue(result1 is Resource.Success)
        val weights1 = (result1 as Resource.Success).data
        assertTrue(weights1.isNotEmpty())
        assertTrue(weights1.containsKey("cheek_fullness"))
        assertTrue(weights1.containsKey("jaw_width"))

        // Verify all weights are in valid range [0.0, 1.0]
        weights1.values.forEach { weight ->
            assertTrue(weight in 0.0f..1.0f)
        }

        // Test dynamic calculation: different landmark positions yield different weights
        val landmarks2 = listOf(
            Pair(10f, 30f),
            Pair(90f, 30f),
            Pair(50f, 60f),
            Pair(20f, 80f),
            Pair(80f, 80f)
        )

        val solver2 = PhotoToCharacterSolver(landmarkDetector = { landmarks2 })
        val result2 = solver2.solveFromImage(mockBitmap, baseMesh)

        assertTrue(result2 is Resource.Success)
        val weights2 = (result2 as Resource.Success).data

        // Weights must be dynamic and non-static
        assertNotEquals(weights1, weights2)
    }

    @Test
    fun testSolveFromImageWhenNoFaceDetected() = runTest {
        val solver = PhotoToCharacterSolver(landmarkDetector = { emptyList() })
        val result = solver.solveFromImage(mockBitmap, baseMesh)

        assertTrue(result is Resource.Error)
        val exception = (result as Resource.Error).exception
        assertTrue(exception is IllegalArgumentException)
        assertEquals("No face detected in photo", exception.message)
    }

    @Test
    fun testSolveFromImageErrorHandling() = runTest {
        val solver = PhotoToCharacterSolver(landmarkDetector = {
            throw IllegalStateException("Detection failed")
        })
        val result = solver.solveFromImage(mockBitmap, baseMesh)

        assertTrue(result is Resource.Error)
        val exception = (result as Resource.Error).exception
        assertTrue(exception is IllegalStateException)
        assertEquals("Detection failed", exception.message)
    }

    @Test
    fun testSolveFromImageCustomMeshMorphTargets() = runTest {
        val customMesh = Mesh(
            id = "custom_mesh",
            name = "CustomMesh",
            vertices = emptyList(),
            normals = emptyList(),
            uvs = emptyList(),
            indices = emptyList(),
            morphTargets = listOf(
                MorphTarget("custom_cheek", mapOf(0 to Vector3(0.1f, 0.0f, 0.0f))),
                MorphTarget("custom_jaw", mapOf(1 to Vector3(0.2f, 0.0f, 0.0f)))
            )
        )

        val landmarks = listOf(
            Pair(30f, 30f),
            Pair(70f, 30f),
            Pair(50f, 50f)
        )

        val solver = PhotoToCharacterSolver(landmarkDetector = { landmarks })
        val result = solver.solveFromImage(mockBitmap, customMesh)

        assertTrue(result is Resource.Success)
        val weights = (result as Resource.Success).data
        assertEquals(2, weights.size)
        assertTrue(weights.containsKey("custom_cheek"))
        assertTrue(weights.containsKey("custom_jaw"))
    }
}
