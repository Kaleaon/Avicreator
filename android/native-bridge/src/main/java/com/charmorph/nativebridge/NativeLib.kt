package com.charmorph.nativebridge

import java.nio.ByteBuffer

class NativeLib {
    companion object {
        private var isLoaded = false

        init {
            try {
                System.loadLibrary("charmorph-native")
                isLoaded = true
            } catch (e: UnsatisfiedLinkError) {
                isLoaded = false
            }
        }
    }

    external fun stringFromJNI(): String

    external fun solveMorphWeights(
        landmarks: FloatArray,
        baseVertices: FloatArray,
        morphIndices: IntArray,
        morphDeltas: FloatArray
    ): FloatArray

    // New Mesh API
    external fun createMesh(vertices: FloatArray): Long

    external fun destroyMesh(meshPtr: Long)

    external fun addMorphTarget(meshPtr: Long, morphId: Int, indices: IntArray, deltas: FloatArray)

    /**
     * Updates the mesh with new morph weights and writes the result into the outputBuffer.
     * outputBuffer must be a DirectByteBuffer with capacity >= vertex_count * 3 * 4 bytes.
     */
    external fun updateMorphs(
        meshPtr: Long,
        morphIds: IntArray,
        morphWeights: FloatArray,
        outputBuffer: ByteBuffer
    )

    external fun parseSkeletonJsonNative(jsonStr: String): String?
    external fun parseManifestJsonNative(jsonStr: String): String?

    fun parseSkeletonJson(jsonStr: String): String? {
        return if (isLoaded) {
            parseSkeletonJsonNative(jsonStr)
        } else {
            jsonStr
        }
    }

    fun parseManifestJson(jsonStr: String): String? {
        return if (isLoaded) {
            parseManifestJsonNative(jsonStr)
        } else {
            jsonStr
        }
    }
}
