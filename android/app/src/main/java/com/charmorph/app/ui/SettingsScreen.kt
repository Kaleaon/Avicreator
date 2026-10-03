package com.charmorph.app.ui

import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.charmorph.storage.SettingsRepository
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val settingsRepository: SettingsRepository
) : ViewModel() {
    val showAnatomicalDetails = settingsRepository.showAnatomicalDetails

    fun toggleAnatomicalDetails(show: Boolean) {
        // In a real ViewModel we'd use viewModelScope and interact with repo
        // But since the repo function is suspend, we'd launch it.
        // For simplicity here, we assume the view model exposes a suspend function or the view calls it.
        // Actually, better to launch here.
    }

    suspend fun updateAnatomicalDetails(show: Boolean) {
        settingsRepository.setShowAnatomicalDetails(show)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    viewModel: SettingsViewModel = hiltViewModel()
) {
    val showDetails by viewModel.showAnatomicalDetails.collectAsState(initial = false)
    val scope = rememberCoroutineScope()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { paddingValues ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
                .padding(16.dp)
        ) {
            Text("Content Preferences", style = MaterialTheme.typography.titleMedium)
            Spacer(modifier = Modifier.height(16.dp))

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text("Show Anatomical Details", style = MaterialTheme.typography.bodyLarge)
                    Text(
                        "Enable detailed anatomical accuracy (e.g. genitalia)",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                Switch(
                    checked = showDetails,
                    onCheckedChange = {
                        scope.launch {
                            viewModel.updateAnatomicalDetails(it)
                        }
                    }
                )
            }
        }
    }
}
