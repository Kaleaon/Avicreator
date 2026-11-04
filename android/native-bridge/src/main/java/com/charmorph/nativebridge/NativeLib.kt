package com.charmorph.nativebridge

import java.nio.ByteBuffer

class NativeLib {
    companion object {
        init {
            try {
                System.loadLibrary("charmorph-native")
            } catch (e: UnsatisfiedLinkError) {
                // Native library not loaded in desktop unit tests
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
}
