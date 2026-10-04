package com.charmorph.app.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Face
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.charmorph.core.model.CharacterMetadata

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    onImportClick: () -> Unit,
    onPhotoImportClick: () -> Unit,
    onSettingsClick: () -> Unit,
    onCharacterClick: (String) -> Unit,
    viewModel: HomeViewModel = hiltViewModel()
) {
    val characters by viewModel.characters.collectAsState()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("CharMorph Library") },
                actions = {
                    IconButton(onClick = onSettingsClick) {
                        Icon(Icons.Default.Settings, contentDescription = "Settings")
                    }
                }
            )
        },
        floatingActionButton = {
            Column(horizontalAlignment = Alignment.End) {
                SmallFloatingActionButton(
                    onClick = onPhotoImportClick,
                    containerColor = MaterialTheme.colorScheme.secondaryContainer,
                    contentColor = MaterialTheme.colorScheme.onSecondaryContainer
                ) {
                    Icon(Icons.Default.Face, contentDescription = "Photo Import")
                }
                Spacer(modifier = Modifier.height(16.dp))
                FloatingActionButton(onClick = onImportClick) {
                    Icon(Icons.Default.Add, contentDescription = "Import Character")
                }
            }
        }
    ) { paddingValues ->
        if (characters.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(paddingValues),
                contentAlignment = Alignment.Center
            ) {
                Text("No characters yet. Tap + to start.", style = MaterialTheme.typography.bodyLarge)
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(paddingValues),
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                items(characters) { character ->
                    CharacterListItem(character, onCharacterClick)
                }
            }
        }
    }
}

@Composable
fun CharacterListItem(character: CharacterMetadata, onClick: (String) -> Unit) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick(character.id) }
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(character.name, style = MaterialTheme.typography.titleMedium)
            Text("ID: ${character.id.take(8)}", style = MaterialTheme.typography.bodySmall)
        }
    }
}
