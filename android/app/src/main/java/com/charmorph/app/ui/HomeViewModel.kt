package com.charmorph.app.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.charmorph.core.model.CharacterMetadata
import com.charmorph.storage.CharacterRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

@HiltViewModel
class HomeViewModel @Inject constructor(
    private val repository: CharacterRepository
) : ViewModel() {

    val characters: StateFlow<List<CharacterMetadata>> = repository.allCharacterMetadata
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())
}
