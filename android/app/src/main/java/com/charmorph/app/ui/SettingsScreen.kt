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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.charmorph.app.ui.theme.ThemeRepository
import com.charmorph.storage.SettingsRepository
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val settingsRepository: SettingsRepository,
    private val themeRepository: ThemeRepository
) : ViewModel() {
    val showAnatomicalDetails = settingsRepository.showAnatomicalDetails
    val activePresetId = themeRepository.activePresetId
    val availableThemes = themeRepository.getAvailablePresets()

    fun toggleAnatomicalDetails(show: Boolean) {
    }

    suspend fun updateAnatomicalDetails(show: Boolean) {
        settingsRepository.setShowAnatomicalDetails(show)
    }

    suspend fun selectThemePreset(presetId: String) {
        themeRepository.selectPreset(presetId)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    viewModel: SettingsViewModel = hiltViewModel()
) {
    val showDetails by viewModel.showAnatomicalDetails.collectAsState(initial = false)
    val activePreset by viewModel.activePresetId.collectAsState(initial = "navy-gold")
    val scope = rememberCoroutineScope()
    var expanded by remember { mutableStateOf(false) }

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

            Spacer(modifier = Modifier.height(24.dp))
            Text("Theme & Styling", style = MaterialTheme.typography.titleMedium)
            Spacer(modifier = Modifier.height(16.dp))

            ExposedDropdownMenuBox(
                expanded = expanded,
                onExpandedChange = { expanded = !expanded }
            ) {
                OutlinedTextField(
                    value = viewModel.availableThemes.find { it.metadata.id == activePreset }?.metadata?.name ?: activePreset,
                    onValueChange = {},
                    readOnly = true,
                    label = { Text("Active Theme Preset") },
                    trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
                    modifier = Modifier.menuAnchor().fillMaxWidth()
                )
                ExposedDropdownMenu(
                    expanded = expanded,
                    onDismissRequest = { expanded = false }
                ) {
                    viewModel.availableThemes.forEach { theme ->
                        DropdownMenuItem(
                            text = { Text(theme.metadata.name) },
                            onClick = {
                                scope.launch {
                                    viewModel.selectThemePreset(theme.metadata.id)
                                }
                                expanded = false
                            }
                        )
                    }
                }
            }
        }
    }
}
