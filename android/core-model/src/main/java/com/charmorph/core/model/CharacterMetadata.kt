package com.charmorph.core.model

import kotlinx.serialization.Serializable

@Serializable
data class CharacterMetadata(
    val id: String,
    val name: String,
    val thumbnailPath: String? = null,
    val lastModified: Long = System.currentTimeMillis(),
    val activeMorphs: Map<String, Float> = emptyMap()
)
