package com.charmorph.storage.entity

import androidx.room.Entity
import androidx.room.PrimaryKey
import androidx.room.TypeConverters

@Entity(tableName = "character_metadata")
@TypeConverters(Converters::class)
data class CharacterMetadataEntity(
    @PrimaryKey val id: String,
    val name: String,
    val thumbnailPath: String?,
    val lastModified: Long,
    val morphWeights: Map<String, Float>
)
