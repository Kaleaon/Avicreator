package com.charmorph.storage.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import com.charmorph.storage.entity.CharacterGeometryEntity
import com.charmorph.storage.entity.CharacterMetadataEntity
import kotlinx.coroutines.flow.Flow

@Dao
interface CharacterDao {
    @Query("SELECT * FROM character_metadata ORDER BY lastModified DESC")
    fun getAllMetadata(): Flow<List<CharacterMetadataEntity>>

    @Query("SELECT * FROM character_metadata WHERE id = :id")
    suspend fun getMetadataById(id: String): CharacterMetadataEntity?

    @Query("SELECT * FROM character_geometry WHERE characterId = :id")
    suspend fun getGeometryById(id: String): CharacterGeometryEntity?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertMetadata(metadata: CharacterMetadataEntity)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertGeometry(geometry: CharacterGeometryEntity)

    @Transaction
    suspend fun insertCharacter(metadata: CharacterMetadataEntity, geometry: CharacterGeometryEntity) {
        insertMetadata(metadata)
        insertGeometry(geometry)
    }

    @Query("UPDATE character_metadata SET morphWeights = :weights, lastModified = :lastModified WHERE id = :id")
    suspend fun updateMorphWeights(id: String, weights: Map<String, Float>, lastModified: Long)

    @Query("DELETE FROM character_metadata WHERE id = :id")
    suspend fun deleteCharacter(id: String)
}
