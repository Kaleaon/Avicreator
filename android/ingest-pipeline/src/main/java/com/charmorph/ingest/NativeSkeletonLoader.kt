package com.charmorph.ingest

import com.charmorph.core.model.Bone
import com.charmorph.core.model.Skeleton
import com.charmorph.core.model.Vector3
import com.charmorph.core.model.Vector4
import com.charmorph.nativebridge.NativeLib
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.int
import kotlinx.serialization.json.float

object NativeSkeletonLoader {
    private val json = Json { ignoreUnknownKeys = true }

    /**
     * Loads a Skeleton object from a JSON string using JNI/FFI native validation & parsing.
     */
    fun loadFromJson(jsonStr: String): Skeleton {
        val nativeLib = NativeLib()
        val parsedJson = nativeLib.parseSkeletonJson(jsonStr) ?: jsonStr
        return parseCanonicalSkeletonJson(parsedJson)
    }

    /**
     * Parses standard avicreator-schema Skeleton JSON string into a Kotlin Skeleton domain object.
     */
    fun parseCanonicalSkeletonJson(canonicalJson: String): Skeleton {
        val rootObj = json.parseToJsonElement(canonicalJson).jsonObject
        val nodesArray = rootObj["nodes"]?.jsonArray ?: return Skeleton(emptyList())

        val bones = nodesArray.map { nodeElement ->
            val nodeObj = nodeElement.jsonObject
            val id = runCatching { nodeObj["id"]?.jsonPrimitive?.int }.getOrNull() ?: 0
            val name = runCatching { nodeObj["name"]?.jsonPrimitive?.content }.getOrNull() ?: "Bone_$id"
            val parentId = runCatching { nodeObj["parent_id"]?.jsonPrimitive?.int }.getOrNull() ?: -1

            val transArr = nodeObj["translation"]?.jsonArray
            val posX = transArr?.get(0)?.jsonPrimitive?.float ?: 0f
            val posY = transArr?.get(1)?.jsonPrimitive?.float ?: 0f
            val posZ = transArr?.get(2)?.jsonPrimitive?.float ?: 0f

            val rotArr = nodeObj["rotation"]?.jsonArray
            val rotX = rotArr?.get(0)?.jsonPrimitive?.float ?: 0f
            val rotY = rotArr?.get(1)?.jsonPrimitive?.float ?: 0f
            val rotZ = rotArr?.get(2)?.jsonPrimitive?.float ?: 0f
            val rotW = rotArr?.get(3)?.jsonPrimitive?.float ?: 1f

            val scaleArr = nodeObj["scale"]?.jsonArray
            val scaleX = scaleArr?.get(0)?.jsonPrimitive?.float ?: 1f
            val scaleY = scaleArr?.get(1)?.jsonPrimitive?.float ?: 1f
            val scaleZ = scaleArr?.get(2)?.jsonPrimitive?.float ?: 1f

            Bone(
                id = id,
                name = name,
                parentId = parentId,
                localPosition = Vector3(posX, posY, posZ),
                localRotation = Vector4(rotX, rotY, rotZ, rotW),
                localScale = Vector3(scaleX, scaleY, scaleZ)
            )
        }

        return Skeleton(bones)
    }

    /**
     * Replaces standard biped stub with native schema deserialization.
     */
    fun createStandardBiped(): Skeleton {
        val defaultBipedJson = """
        {
            "name": "StandardBiped",
            "nodes": [
                {"id": 0, "name": "Root", "parent_id": null, "translation": [0.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]},
                {"id": 1, "name": "Pelvis", "parent_id": 0, "translation": [0.0, 1.0, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]},
                {"id": 2, "name": "Spine", "parent_id": 1, "translation": [0.0, 0.2, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]},
                {"id": 3, "name": "Head", "parent_id": 2, "translation": [0.0, 0.4, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]}
            ],
            "root_indices": [0]
        }
        """.trimIndent()
        return loadFromJson(defaultBipedJson)
    }
}
