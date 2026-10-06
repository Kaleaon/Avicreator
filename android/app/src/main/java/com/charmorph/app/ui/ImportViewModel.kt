package com.charmorph.app.ui

import android.content.Context
import android.net.Uri
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.charmorph.core.model.Character
import com.charmorph.core.model.Skeleton
import com.charmorph.ingest.ObjParser
import com.charmorph.storage.CharacterRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.UUID
import javax.inject.Inject

@HiltViewModel
class ImportViewModel @Inject constructor(
    private val repository: CharacterRepository,
    @ApplicationContext private val context: Context
) : ViewModel() {

    fun importTestCharacter() {
        viewModelScope.launch {
            // Use the TestAssetGenerator which creates a cube
            val mesh = com.charmorph.asset.TestAssetGenerator.createCubeMesh()
            saveCharacter(mesh, "Test Cube")
        }
    }

    fun importFromUri(uri: Uri) {
        viewModelScope.launch {
             withContext(Dispatchers.IO) {
                 try {
                     context.contentResolver.openInputStream(uri)?.use { stream ->
                         val mesh = ObjParser.parse(stream, "Imported Mesh")
                         saveCharacter(mesh, "Imported Character")
                     }
                 } catch (e: Exception) {
                     e.printStackTrace()
                 }
             }
        }
    }

    private suspend fun saveCharacter(mesh: com.charmorph.core.model.Mesh, name: String) {
        val character = Character(
            id = UUID.randomUUID().toString(),
            baseMesh = mesh.copy(name = name),
            skeleton = Skeleton(emptyList()),
            activeMorphs = mapOf("body_fat" to 0.5f)
        )
        repository.saveCharacter(character)
    }
}
