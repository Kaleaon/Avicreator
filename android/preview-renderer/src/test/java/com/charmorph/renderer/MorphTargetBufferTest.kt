package com.charmorph.renderer

import com.charmorph.core.model.Mesh
import com.charmorph.core.model.MorphTarget
import com.charmorph.core.model.Vector3
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class MorphTargetBufferTest {

    @Test
    fun testMorphTargetDataStructure() {
        val deltas = mapOf(
            0 to Vector3(0.1f, 0.2f, 0.3f),
            5 to Vector3(-0.1f, 0.0f, 0.5f)
        )
        val target = MorphTarget(
            name = "nose_length",
            deltas = deltas,
            minValue = -1.0f,
            maxValue = 1.0f
        )

        assertEquals("nose_length", target.name)
        assertEquals(2, target.deltas.size)
        assertEquals(0.1f, target.deltas[0]?.x)
        assertEquals(-1.0f, target.minValue)
        assertEquals(1.0f, target.maxValue)
    }

    @Test
    fun testMeshWithMorphTargets() {
        val mesh = Mesh(
            id = "mesh_01",
            name = "BaseMesh",
            vertices = listOf(Vector3(0f, 0f, 0f), Vector3(1f, 1f, 1f)),
            normals = emptyList(),
            uvs = emptyList(),
            indices = listOf(0, 1, 0),
            morphTargets = listOf(
                MorphTarget("breast_size", mapOf(0 to Vector3(0.0f, 0.1f, 0.0f))),
                MorphTarget("waist_width", mapOf(1 to Vector3(0.05f, 0.0f, 0.0f)))
            )
        )

        assertEquals(2, mesh.morphTargets.size)
        assertEquals("breast_size", mesh.morphTargets[0].name)
        assertEquals("waist_width", mesh.morphTargets[1].name)
    }

    @Test
    fun testSparseDeltaFlattening() {
        val vertexCount = 10
        val deltasMap = mapOf(
            2 to Vector3(1.5f, 2.5f, 3.5f)
        )

        val flattened = FloatArray(vertexCount * 3)
        deltasMap.forEach { (vIdx, delta) ->
            if (vIdx in 0 until vertexCount) {
                flattened[vIdx * 3] = delta.x
                flattened[vIdx * 3 + 1] = delta.y
                flattened[vIdx * 3 + 2] = delta.z
            }
        }

        assertEquals(1.5f, flattened[6], 0.001f)
        assertEquals(2.5f, flattened[7], 0.001f)
        assertEquals(3.5f, flattened[8], 0.001f)
        assertEquals(0.0f, flattened[0], 0.001f)
    }
}
