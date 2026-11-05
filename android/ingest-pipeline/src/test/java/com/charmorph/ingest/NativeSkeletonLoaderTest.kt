package com.charmorph.ingest

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class NativeSkeletonLoaderTest {

    private val loader = NativeSkeletonLoader()

    @Test
    fun testLoadSkeletonFromNullReturnsStandardBiped() {
        val skeleton = loader.loadSkeletonFromJson(null)
        assertNotNull(skeleton)
        assertEquals(4, skeleton.bones.size)
        assertEquals("Root", skeleton.bones[0].name)
    }

    @Test
    fun testLoadSkeletonFromEmptyStringReturnsStandardBiped() {
        val skeleton = loader.loadSkeletonFromJson("")
        assertNotNull(skeleton)
        assertEquals(4, skeleton.bones.size)
    }

    @Test
    fun testLoadSkeletonFromValidJson() {
        val json = """
            {
              "bones": [
                {
                  "id": 0,
                  "name": "CustomRoot",
                  "parent_id": -1,
                  "position": [0.0, 0.0, 0.0],
                  "rotation": [0.0, 0.0, 0.0, 1.0],
                  "scale": [1.0, 1.0, 1.0]
                },
                {
                  "id": 1,
                  "name": "CustomSpine",
                  "parent_id": 0,
                  "position": [0.0, 1.0, 0.0],
                  "rotation": [0.0, 0.0, 0.0, 1.0],
                  "scale": [1.0, 1.0, 1.0]
                }
              ]
            }
        """.trimIndent()

        val skeleton = loader.loadSkeletonFromJson(json)
        assertNotNull(skeleton)
        assertEquals(2, skeleton.bones.size)
        assertEquals("CustomRoot", skeleton.bones[0].name)
        assertEquals("CustomSpine", skeleton.bones[1].name)
        assertEquals(0, skeleton.bones[1].parentId)
    }

    @Test
    fun testParseManifestJsonValid() {
        val json = """
            {
              "author": "CharMorph",
              "version": "1.0.0"
            }
        """.trimIndent()

        val manifest = loader.parseManifestJson(json)
        assertEquals("CharMorph", manifest["author"])
        assertEquals("1.0.0", manifest["version"])
    }

    @Test
    fun testParseManifestJsonNullOrInvalid() {
        val manifestNull = loader.parseManifestJson(null)
        assertEquals(0, manifestNull.size)

        val manifestInvalid = loader.parseManifestJson("invalid_json")
        assertEquals(0, manifestInvalid.size)
    }
}
