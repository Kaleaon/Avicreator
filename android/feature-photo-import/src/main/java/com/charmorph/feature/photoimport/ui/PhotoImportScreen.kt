package com.charmorph.feature.photoimport.ui

import android.graphics.BitmapFactory
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Resource
import com.charmorph.feature.photoimport.PhotoToCharacterSolver
import kotlinx.coroutines.launch

@Composable
fun PhotoImportScreen(
    onBack: () -> Unit,
    solver: PhotoToCharacterSolver = remember { PhotoToCharacterSolver() },
    baseMesh: Mesh = remember { Mesh("base_mesh", "BaseMesh", emptyList(), emptyList(), emptyList(), emptyList()) }
) {
    var status by remember { mutableStateOf("Select a photo to start") }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    val launcher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent()
    ) { uri: Uri? ->
        if (uri != null) {
            status = "Analyzing photo..."
            scope.launch {
                val bitmap = try {
                    context.contentResolver.openInputStream(uri)?.use { stream ->
                        BitmapFactory.decodeStream(stream)
                    }
                } catch (e: Exception) {
                    null
                }

                if (bitmap == null) {
                    status = "Error: Failed to decode image"
                    return@launch
                }

                when (val result = solver.solveFromImage(bitmap, baseMesh)) {
                    is Resource.Success -> {
                        val weightsStr = result.data.entries.joinToString("\n") { (key, weight) ->
                            "$key: ${"%.2f".format(weight)}"
                        }
                        status = "Calculated Morph Weights:\n$weightsStr"
                    }
                    is Resource.Error -> {
                        status = "Error: ${result.exception.message ?: "No face detected in photo"}"
                    }
                    is Resource.Loading -> {
                        status = "Analyzing photo..."
                    }
                }
            }
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
