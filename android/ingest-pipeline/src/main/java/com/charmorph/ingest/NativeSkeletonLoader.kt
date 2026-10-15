package com.charmorph.ingest

import com.charmorph.core.model.Bone
import com.charmorph.core.model.Skeleton
import com.charmorph.core.model.Vector3
import com.charmorph.core.model.Vector4
import com.charmorph.nativebridge.NativeLib
import org.json.JSONArray
import org.json.JSONObject

class NativeSkeletonLoader(
    private val nativeLib: NativeLib = NativeLib()
) {
    fun loadSkeletonFromJson(jsonStr: String?): Skeleton {
        if (jsonStr.isNullOrEmpty()) {
            return GltfSkeletonStub.createStandardBiped()
        }

        val textToParse = try {
            nativeLib.parseSkeletonJsonNative(jsonStr)
        } catch (e: Throwable) {
            jsonStr
        }

        return parseSkeletonJsonString(textToParse)
    }

    fun parseManifestJson(jsonStr: String?): Map<String, String> {
        if (jsonStr.isNullOrEmpty()) {
            return emptyMap()
        }

        val textToParse = try {
            nativeLib.parseManifestJsonNative(jsonStr)
        } catch (e: Throwable) {
            jsonStr
        }

        return try {
            val jsonObject = JSONObject(textToParse)
            val resultMap = mutableMapOf<String, String>()
            jsonObject.keys().forEach { key ->
                resultMap[key] = jsonObject.optString(key, "")
            }
            resultMap
        } catch (e: Throwable) {
            emptyMap()
        }
    }

    private fun parseSkeletonJsonString(jsonStr: String): Skeleton {
        return try {
            val jsonObject = JSONObject(jsonStr)
            val bonesArray = jsonObject.optJSONArray("bones") ?: JSONArray()
            val boneList = mutableListOf<Bone>()

            for (i in 0 until bonesArray.length()) {
                val bObj = bonesArray.getJSONObject(i)
                val id = bObj.optInt("id", i)
                val name = bObj.optString("name", "Bone_$id")
                val parentId = bObj.optInt("parent_id", -1)

                val posArray = bObj.optJSONArray("position")
                val position = if (posArray != null && posArray.length() >= 3) {
                    Vector3(
                        posArray.getDouble(0).toFloat(),
                        posArray.getDouble(1).toFloat(),
                        posArray.getDouble(2).toFloat()
                    )
                } else Vector3(0f, 0f, 0f)

                val rotArray = bObj.optJSONArray("rotation")
                val rotation = if (rotArray != null && rotArray.length() >= 4) {
                    Vector4(
                        rotArray.getDouble(0).toFloat(),
                        rotArray.getDouble(1).toFloat(),
                        rotArray.getDouble(2).toFloat(),
                        rotArray.getDouble(3).toFloat()
                    )
                } else Vector4(0f, 0f, 0f, 1f)

                val scaleArray = bObj.optJSONArray("scale")
                val scale = if (scaleArray != null && scaleArray.length() >= 3) {
                    Vector3(
                        scaleArray.getDouble(0).toFloat(),
                        scaleArray.getDouble(1).toFloat(),
                        scaleArray.getDouble(2).toFloat()
                    )
                } else Vector3(1f, 1f, 1f)

                boneList.add(Bone(id, name, parentId, position, rotation, scale))
            }

            if (boneList.isEmpty()) {
                GltfSkeletonStub.createStandardBiped()
            } else {
                Skeleton(boneList)
            }
        } catch (e: Throwable) {
            e.printStackTrace()
            GltfSkeletonStub.createStandardBiped()
        }
    }
}
