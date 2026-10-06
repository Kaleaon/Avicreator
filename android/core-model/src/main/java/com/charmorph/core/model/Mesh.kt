package com.charmorph.core.model

import kotlinx.serialization.Serializable

@Serializable
data class Vector3(val x: Float, val y: Float, val z: Float)

@Serializable
data class Vector4(val x: Float, val y: Float, val z: Float, val w: Float)

@Serializable
data class Vector2(val u: Float, val v: Float)

@Serializable
data class Matrix4(
    val values: List<Float> // 16 floats, column-major
)

@Serializable
data class MeshGroup(
    val name: String,
    val indices: List<Int>,
    val materialIndex: Int,
    val tags: List<String> = emptyList() // e.g. "anatomical", "genitalia"
)

@Serializable
data class SkinData(
    val jointIndices: List<Vector4>, // 4 ints per vertex (stored as floats for simplicity in serialization, or Int Vector4)
    val weights: List<Vector4>       // 4 weights per vertex
)

@Serializable
data class Mesh(
    val id: String,
    val name: String,
    val vertices: List<Vector3>,
    val normals: List<Vector3>,
    val uvs: List<Vector2>,
    val indices: List<Int>,
    val groups: List<MeshGroup> = emptyList(),
    val skinData: SkinData? = null, // Optional skinning data
    val morphTargets: List<MorphTarget> = emptyList()
)

@Serializable
data class MorphTarget(
    val name: String,
    val deltas: Map<Int, Vector3>, // Map vertex index to delta vector for sparse storage
    val minValue: Float = 0f,
    val maxValue: Float = 1f
)

@Serializable
data class Bone(
    val id: Int,
    val name: String,
    val parentId: Int, // -1 if root
    // Transform relative to parent (Bind Pose)
    val localPosition: Vector3,
    val localRotation: Vector4, // Quaternion (x,y,z,w)
    val localScale: Vector3,
    // Inverse Bind Matrix (Model Space -> Bone Space)
    val inverseBindMatrix: Matrix4? = null
)

@Serializable
data class Skeleton(
    val bones: List<Bone>
)

@Serializable
data class Character(
    val id: String,
    val baseMesh: Mesh,
    val skeleton: Skeleton,
    val activeMorphs: Map<String, Float> = emptyMap()
)
