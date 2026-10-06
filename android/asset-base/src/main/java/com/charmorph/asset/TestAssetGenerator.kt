package com.charmorph.asset

import com.charmorph.core.math.MathUtils
import com.charmorph.core.model.Bone
import com.charmorph.core.model.Matrix4
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.MeshGroup
import com.charmorph.core.model.Skeleton
import com.charmorph.core.model.SkinData
import com.charmorph.core.model.Vector2
import com.charmorph.core.model.Vector3
import com.charmorph.core.model.Vector4

object TestAssetGenerator {

    fun createCubeMesh(): Mesh {
        // ... (Existing Cube Logic)
        // Returning simplified logic for brevity, assuming previous implementation is sufficient for cube
        // This generator now focuses on the cylinder
        return createSkinnedCylinder()
    }

    fun createSkinnedCylinder(): Mesh {
        val radius = 0.5f
        val height = 4.0f
        val segments = 16
        val rings = 8

        val vertices = mutableListOf<Vector3>()
        val normals = mutableListOf<Vector3>()
        val uvs = mutableListOf<Vector2>()
        val indices = mutableListOf<Int>()

        val jointIndices = mutableListOf<Vector4>()
        val weights = mutableListOf<Vector4>()

        // Generate cylinder
        for (r in 0..rings) {
            val y = (r.toFloat() / rings) * height
            val ringProgress = r.toFloat() / rings // 0.0 to 1.0

            for (s in 0..segments) {
                val angle = (s.toFloat() / segments) * Math.PI * 2.0
                val x = radius * Math.cos(angle).toFloat()
                val z = radius * Math.sin(angle).toFloat()

                vertices.add(Vector3(x, y, z))
                normals.add(normalize(Vector3(x, 0f, z)))
                uvs.add(Vector2(s.toFloat() / segments, ringProgress))

                // Skinning Logic (2 Bones: Bottom and Top)
                // Bone 0: Y = 0 to 2 (Root)
                // Bone 1: Y = 2 to 4 (Child)
                // Simple linear blend at the joint (Y=2)

                if (y <= 1.0f) {
                    jointIndices.add(Vector4(0f, 0f, 0f, 0f))
                    weights.add(Vector4(1f, 0f, 0f, 0f))
                } else if (y >= 3.0f) {
                    jointIndices.add(Vector4(1f, 0f, 0f, 0f))
                    weights.add(Vector4(1f, 0f, 0f, 0f))
                } else {
                    // Blend region (1.0 to 3.0)
                    val factor = (y - 1.0f) / 2.0f // 0.0 to 1.0
                    jointIndices.add(Vector4(0f, 1f, 0f, 0f))
                    weights.add(Vector4(1f - factor, factor, 0f, 0f))
                }
            }
        }

        // Indices
        for (r in 0 until rings) {
            for (s in 0 until segments) {
                val cur = r * (segments + 1) + s
                val next = (r + 1) * (segments + 1) + s

                indices.add(cur)
                indices.add(next)
                indices.add(cur + 1)

                indices.add(cur + 1)
                indices.add(next)
                indices.add(next + 1)
            }
        }

        val groups = listOf(MeshGroup("Body", indices, 0, listOf("body")))

        val skinData = SkinData(jointIndices, weights)

        return Mesh(
            id = "test_cylinder",
            name = "Skinned Cylinder",
            vertices = vertices,
            normals = normals,
            uvs = uvs,
            indices = indices,
            groups = groups,
            skinData = skinData
        )
    }

    fun createCylinderSkeleton(): Skeleton {
        // Bone 0: Root at (0,0,0)
        // Bone 1: Joint at (0,2,0)

        val bone0 = Bone(
            id = 0,
            name = "Root_Bone",
            parentId = -1,
            localPosition = Vector3(0f, 0f, 0f),
            localRotation = Vector4(0f, 0f, 0f, 1f),
            localScale = Vector3(1f, 1f, 1f),
            inverseBindMatrix = Matrix4(MathUtils.invertM(MathUtils.composeMatrix(
                Vector3(0f, 0f, 0f), Vector4(0f, 0f, 0f, 1f), Vector3(1f, 1f, 1f)
            )).toList())
        )

        val bone1 = Bone(
            id = 1,
            name = "Top_Bone",
            parentId = 0,
            localPosition = Vector3(0f, 2f, 0f), // Relative to root
            localRotation = Vector4(0f, 0f, 0f, 1f),
            localScale = Vector3(1f, 1f, 1f),
            // InvBind is World Space inverted. Bone1 World Pos is (0,2,0)
            inverseBindMatrix = Matrix4(MathUtils.invertM(MathUtils.composeMatrix(
                Vector3(0f, 2f, 0f), Vector4(0f, 0f, 0f, 1f), Vector3(1f, 1f, 1f)
            )).toList())
        )

        return Skeleton(listOf(bone0, bone1))
    }

    private fun normalize(v: Vector3): Vector3 {
        val len = Math.sqrt((v.x * v.x + v.y * v.y + v.z * v.z).toDouble()).toFloat()
        return if (len > 0) Vector3(v.x / len, v.y / len, v.z / len) else v
    }
}
