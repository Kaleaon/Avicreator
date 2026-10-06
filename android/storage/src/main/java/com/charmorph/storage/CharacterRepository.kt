package com.charmorph.storage

import com.charmorph.core.model.Character
import com.charmorph.core.model.CharacterMetadata
import com.charmorph.storage.dao.CharacterDao
import com.charmorph.storage.entity.CharacterGeometryEntity
import com.charmorph.storage.entity.CharacterMetadataEntity
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
open class CharacterRepository @Inject constructor(
    private val characterDao: CharacterDao
) {
    open val allCharacterMetadata: Flow<List<CharacterMetadata>> = characterDao.getAllMetadata().map { entities ->
        entities.map { it.toDomainModel() }
    }

    // Alias for backward compatibility if needed
    open val allCharacters: Flow<List<CharacterMetadata>> = allCharacterMetadata

    open suspend fun getCharacter(id: String): Character? {
        val metadata = characterDao.getMetadataById(id) ?: return null
        val geometry = characterDao.getGeometryById(id) ?: return null
        return Character(
            id = metadata.id,
            baseMesh = geometry.meshData.copy(name = metadata.name),
            skeleton = geometry.skeletonData,
            activeMorphs = metadata.morphWeights
        )
    }

    open suspend fun saveCharacter(character: Character) {
        val metadataEntity = CharacterMetadataEntity(
            id = character.id,
            name = character.baseMesh.name,
            thumbnailPath = null,
            lastModified = System.currentTimeMillis(),
            morphWeights = character.activeMorphs
        )
        val geometryEntity = CharacterGeometryEntity(
            characterId = character.id,
            meshData = character.baseMesh,
            skeletonData = character.skeleton
        )
        characterDao.insertCharacter(metadataEntity, geometryEntity)
    }

    open suspend fun updateMorphWeights(id: String, weights: Map<String, Float>) {
        characterDao.updateMorphWeights(id, weights, System.currentTimeMillis())
    }
}

// Mappers
fun CharacterMetadataEntity.toDomainModel(): CharacterMetadata {
    return CharacterMetadata(
        id = id,
        name = name,
        thumbnailPath = thumbnailPath,
        lastModified = lastModified,
        activeMorphs = morphWeights
    )
}
