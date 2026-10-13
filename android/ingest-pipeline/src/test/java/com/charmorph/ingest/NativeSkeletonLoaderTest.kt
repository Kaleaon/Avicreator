package com.charmorph.ingest

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test

class NativeSkeletonLoaderTest {

    @Test
    fun testCreateStandardBiped() {
        val skeleton = NativeSkeletonLoader.createStandardBiped()
        assertNotNull(skeleton)
        assertEquals(4, skeleton.bones.size)

        val root = skeleton.bones.find { it.name == "Root" }
        assertNotNull(root)
        assertEquals(0, root?.id)
        assertEquals(-1, root?.parentId)

        val pelvis = skeleton.bones.find { it.name == "Pelvis" }
        assertNotNull(pelvis)
        assertEquals(1, pelvis?.id)
        assertEquals(0, pelvis?.parentId)
    }

    @Test
    fun testParseCanonicalSkeletonJson() {
        val jsonStr = """
        {
            "name": "CustomBiped",
            "nodes": [
                {"id": 0, "name": "Hips", "parent_id": null, "translation": [0.0, 1.0, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]},
                {"id": 1, "name": "Chest", "parent_id": 0, "translation": [0.0, 0.5, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]}
            ],
            "root_indices": [0]
        }
        """.trimIndent()

        val skeleton = NativeSkeletonLoader.loadFromJson(jsonStr)
        assertNotNull(skeleton)
        assertEquals(2, skeleton.bones.size)

        val hips = skeleton.bones[0]
        assertEquals("Hips", hips.name)
        assertEquals(-1, hips.parentId)
        assertEquals(1.0f, hips.localPosition.y)

        val chest = skeleton.bones[1]
        assertEquals("Chest", chest.name)
        assertEquals(0, chest.parentId)
        assertEquals(0.5f, chest.localPosition.y)
    }
}
