package com.charmorph.app.ui

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.charmorph.core.math.MathUtils
import com.charmorph.core.model.Character
import com.charmorph.core.model.Vector4
import com.charmorph.renderer.TextureType
import com.charmorph.storage.CharacterRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class BoneState(
    val id: Int,
    val name: String,
    val pitch: Float = 0f,
    val yaw: Float = 0f,
    val roll: Float = 0f
)

data class EditorUiState(
    val morphs: List<MorphState> = emptyList(),
    val bones: List<BoneState> = emptyList(),
    val activeCategory: String = "Body",
    val mode: EditorMode = EditorMode.MORPHS,
    val selectedTextureSlot: TextureType = TextureType.ALBEDO
)

enum class EditorMode {
    MORPHS, POSE, MATERIALS
}

@HiltViewModel
class EditorViewModel @Inject constructor(
    private val repository: CharacterRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val characterId: String = checkNotNull(savedStateHandle["characterId"])

    private val _uiState = MutableStateFlow(EditorUiState())
    val uiState: StateFlow<EditorUiState> = _uiState.asStateFlow()

    private var currentCharacter: Character? = null

    init {
        loadCharacter()
    }

    private fun loadCharacter() {
        viewModelScope.launch {
            val character = repository.getCharacter(characterId)
            if (character != null) {
                currentCharacter = character

                val activeWeights = character.activeMorphs
                val morphs = getAvailableMorphs().map { morph ->
                    morph.copy(value = activeWeights[morph.name] ?: morph.value)
                }

                val bones = character.skeleton.bones.map { bone ->
                    BoneState(bone.id, bone.name)
                }

                _uiState.value = _uiState.value.copy(morphs = morphs, bones = bones)
            }
        }
    }

    fun setCategory(category: String) {
        _uiState.value = _uiState.value.copy(activeCategory = category)
    }

    fun setMode(mode: EditorMode) {
        _uiState.value = _uiState.value.copy(mode = mode)
    }

    fun selectTextureSlot(type: TextureType) {
        _uiState.value = _uiState.value.copy(selectedTextureSlot = type)
    }

    fun updateMorph(name: String, value: Float) {
        val currentMorphs = _uiState.value.morphs.toMutableList()
        val index = currentMorphs.indexOfFirst { it.name == name }
        if (index != -1) {
            currentMorphs[index] = currentMorphs[index].copy(value = value)
            _uiState.value = _uiState.value.copy(morphs = currentMorphs)
            saveCurrentState()
        }
    }

    fun updateBone(boneId: Int, pitch: Float, yaw: Float, roll: Float) {
        val currentBones = _uiState.value.bones.toMutableList()
        val index = currentBones.indexOfFirst { it.id == boneId }
        if (index != -1) {
            currentBones[index] = currentBones[index].copy(pitch = pitch, yaw = yaw, roll = roll)
            _uiState.value = _uiState.value.copy(bones = currentBones)
        }
    }

    fun getBoneRotation(boneId: Int): Vector4? {
        val bone = _uiState.value.bones.find { it.id == boneId } ?: return null
        return MathUtils.eulerToQuaternion(bone.pitch, bone.yaw, bone.roll)
    }

    private fun saveCurrentState() {
        viewModelScope.launch {
            val weights = _uiState.value.morphs.associate { it.name to it.value }
            repository.updateMorphWeights(characterId, weights)
        }
    }

    fun getCharacterMesh() = currentCharacter?.baseMesh
    fun getCharacterSkeleton() = currentCharacter?.skeleton

    private fun getAvailableMorphs(): List<MorphState> {
        return listOf(
            MorphState("body_fat", "Fatness", "Body"),
            MorphState("body_muscle", "Muscle", "Body"),
            MorphState("genital_size", "Size", "Genitalia", min=0f, max=2f),
            MorphState("breast_size", "Breast Size", "Body", min=0f, max=2f)
        )
    }
}
