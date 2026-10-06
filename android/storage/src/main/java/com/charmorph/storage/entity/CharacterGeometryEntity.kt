package com.charmorph.storage.entity

import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.PrimaryKey
import androidx.room.TypeConverters
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Skeleton

@Entity(
    tableName = "character_geometry",
    foreignKeys = [
        ForeignKey(
            entity = CharacterMetadataEntity::class,
            parentColumns = ["id"],
            childColumns = ["characterId"],
            onDelete = ForeignKey.CASCADE
        )
    ]
)
@TypeConverters(Converters::class)
data class CharacterGeometryEntity(
    @PrimaryKey val characterId: String,
    val meshData: Mesh,
    val skeletonData: Skeleton
)
