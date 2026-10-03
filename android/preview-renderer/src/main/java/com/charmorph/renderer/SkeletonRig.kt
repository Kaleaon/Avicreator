package com.charmorph.renderer

import com.charmorph.core.math.MathUtils
import com.charmorph.core.model.Skeleton
import com.charmorph.core.model.Vector4

class SkeletonRig(private val skeleton: Skeleton) {

    private val bones = skeleton.bones
    // Local transforms (mutable state for animation)
    private val localRotations = bones.map { it.localRotation }.toMutableList()
    private val localPositions = bones.map { it.localPosition }.toMutableList()
    private val localScales = bones.map { it.localScale }.toMutableList()

    // Computed global matrices (World Space)
    private val globalMatrices = Array(bones.size) { FloatArray(16) }

    // Skinning matrices (passed to shader: InverseBind * Global)
    private val skinningMatrices = Array(bones.size) { FloatArray(16) }

    // Flattened buffer for Filament (size * 16)
    val skinningBuffer = FloatArray(bones.size * 16)

    init {
        updateGlobalMatrices()
    }

    fun updateBone(boneId: Int, rotation: Vector4) {
        if (boneId in localRotations.indices) {
            localRotations[boneId] = rotation
            updateGlobalMatrices()
        }
    }

    private fun updateGlobalMatrices() {
        // Naive hierarchical update (assumes bones are sorted parent -> child)
        for (i in bones.indices) {
            val bone = bones[i]

            val localMat = MathUtils.composeMatrix(localPositions[i], localRotations[i], localScales[i])

            if (bone.parentId == -1) {
                globalMatrices[i] = localMat
            } else {
                val parentMat = globalMatrices[bone.parentId]
                val result = FloatArray(16)
                MathUtils.multiplyMM(result, parentMat, localMat)
                globalMatrices[i] = result
            }

            // Compute skinning matrix: Global * InvBind
            val invBind = bone.inverseBindMatrix?.values?.toFloatArray() ?: floatArrayOf(
                1f, 0f, 0f, 0f, 0f, 1f, 0f, 0f, 0f, 0f, 1f, 0f, 0f, 0f, 0f, 1f
            )

            val skinMat = FloatArray(16)
            // Note: Standard skinning usually expects Global * InverseBind
            // Order depends on column-major vs row-major conventions.
            // Android OpenGL is column-major.
            MathUtils.multiplyMM(skinMat, globalMatrices[i], invBind)
            skinningMatrices[i] = skinMat

            // Copy to buffer
            System.arraycopy(skinningMatrices[i], 0, skinningBuffer, i * 16, 16)
        }
    }
}
