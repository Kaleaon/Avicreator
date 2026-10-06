package com.charmorph.feature.photoimport.ui

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
fun PhotoImportScreen(onBack: () -> Unit) {
    var status by remember { mutableStateOf("Select a photo to start") }

    val launcher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent()
    ) { uri ->
        if (uri != null) {
            status = "Analyzing photo... (Simulated)"
            // Trigger PhotoToCharacterSolver here
        }
    }

    Column(
        modifier = Modifier.fillMaxSize().padding(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Text("Photo to Character", style = MaterialTheme.typography.headlineMedium)
        Spacer(modifier = Modifier.height(24.dp))
        Text(status)
        Spacer(modifier = Modifier.height(24.dp))
        Button(onClick = { launcher.launch("image/*") }) {
            Text("Pick Photo")
        }
        Spacer(modifier = Modifier.height(16.dp))
        OutlinedButton(onClick = onBack) {
            Text("Cancel")
        }
    }
}
