package com.charmorph.app.ui

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.hilt.navigation.compose.hiltViewModel
import io.ktheme.compose.LocalKtheme
import com.charmorph.renderer.FilamentView
import com.charmorph.renderer.TextureType

data class TextureSlot(
    val type: TextureType,
    val name: String,
    val hasTexture: Boolean = false
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EditorScreen(
    onBack: () -> Unit,
    viewModel: EditorViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    val mesh = viewModel.getCharacterMesh()
    val skeleton = viewModel.getCharacterSkeleton()
    val ktheme = LocalKtheme.current

    val lifecycleOwner = androidx.compose.ui.platform.LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_PAUSE || event == androidx.lifecycle.Lifecycle.Event.ON_STOP) {
                viewModel.flushPendingWrites()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
            viewModel.flushPendingWrites()
        }
    }

    var filamentView: FilamentView? by remember { mutableStateOf(null) }

    val categories = remember(uiState.morphs) {
        uiState.morphs.map { it.category }.distinct().sorted()
    }

    val activeMorphs = remember(uiState.morphs, uiState.activeCategory) {
        uiState.morphs.filter { it.category == uiState.activeCategory }
    }

    val texturePicker = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent()
    ) { uri ->
        if (uri != null && filamentView != null) {
            filamentView?.loadTexture(uri, uiState.selectedTextureSlot)
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Character Editor (${ktheme.metadata.name})") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { paddingValues ->
        Column(modifier = Modifier.padding(paddingValues).fillMaxSize()) {
            // 3D Viewport (taking upper half)
            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .background(MaterialTheme.colorScheme.surfaceVariant)
            ) {
                 if (mesh != null) {
                     AndroidView(
                        modifier = Modifier
                            .fillMaxSize()
                            .semantics { contentDescription = "3D Character Viewport" },
                        factory = { context ->
                            FilamentView(context).apply {
                                loadMesh(mesh, skeleton)
                                filamentView = this
                            }
                        },
                        update = { view ->
                            // Apply Morphs
                            uiState.morphs.forEach { morph ->
                                 view.updateMorphWeight(morph.name, morph.value)
                            }

                            // Apply Pose
                            uiState.bones.forEach { bone ->
                                val quat = viewModel.getBoneRotation(bone.id)
                                if (quat != null) {
                                    view.updateBoneRotation(bone.id, quat)
                                }
                            }
                        }
                     )
                 } else {
                     Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                         CircularProgressIndicator()
                     }
                 }
            }

            // Controls (taking lower half)
            Surface(
                modifier = Modifier.weight(1f),
                tonalElevation = 2.dp
            ) {
                Column {
                    // Mode Tabs
                    TabRow(selectedTabIndex = uiState.mode.ordinal) {
                        Tab(selected = uiState.mode == EditorMode.MORPHS, onClick = { viewModel.setMode(EditorMode.MORPHS) }, text = { Text("Morphs") })
                        Tab(selected = uiState.mode == EditorMode.POSE, onClick = { viewModel.setMode(EditorMode.POSE) }, text = { Text("Pose") })
                        Tab(selected = uiState.mode == EditorMode.MATERIALS, onClick = { viewModel.setMode(EditorMode.MATERIALS) }, text = { Text("Material") })
                    }

                    when (uiState.mode) {
                        EditorMode.MORPHS -> {
                            ScrollableTabRow(
                                selectedTabIndex = categories.indexOf(uiState.activeCategory).coerceAtLeast(0),
                                edgePadding = 16.dp
                            ) {
                                for (category in categories) {
                                    Tab(
                                        selected = category == uiState.activeCategory,
                                        onClick = { viewModel.setCategory(category) },
                                        text = { Text(category) }
                                    )
                                }
                            }

                            LazyColumn(
                                contentPadding = PaddingValues(16.dp),
                                verticalArrangement = Arrangement.spacedBy(16.dp)
                            ) {
                                items(activeMorphs) { morph ->
                                    MorphSlider(
                                        morph = morph,
                                        onValueChange = { viewModel.updateMorph(morph.name, it) }
                                    )
                                }
                            }
                        }
                        EditorMode.POSE -> {
                            LazyColumn(
                                contentPadding = PaddingValues(16.dp),
                                verticalArrangement = Arrangement.spacedBy(16.dp)
                            ) {
                                items(uiState.bones) { bone ->
                                    BoneControl(
                                        bone = bone,
                                        onUpdate = { p, y, r -> viewModel.updateBone(bone.id, p, y, r) }
                                    )
                                }
                            }
                        }
                        EditorMode.MATERIALS -> {
                            Column(modifier = Modifier.padding(16.dp)) {
                                Text("Texture Maps", style = MaterialTheme.typography.titleMedium)
                                Spacer(modifier = Modifier.height(16.dp))

                                Button(onClick = {
                                    viewModel.selectTextureSlot(TextureType.ALBEDO)
                                    texturePicker.launch("image/*")
                                }) {
                                    Text("Load Albedo / Color")
                                }
                                Spacer(modifier = Modifier.height(8.dp))
                                Button(onClick = {
                                    viewModel.selectTextureSlot(TextureType.NORMAL)
                                    texturePicker.launch("image/*")
                                }) {
                                    Text("Load Normal Map")
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun MorphSlider(
    morph: MorphState,
    onValueChange: (Float) -> Unit
) {
    Column {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Text(text = morph.displayName, style = MaterialTheme.typography.bodyMedium)
            Text(text = "%.2f".format(morph.value), style = MaterialTheme.typography.bodySmall)
        }
        Slider(
            value = morph.value,
            onValueChange = onValueChange,
            valueRange = morph.min..morph.max,
            modifier = Modifier.semantics { contentDescription = morph.displayName }
        )
    }
}

@Composable
fun BoneControl(
    bone: BoneState,
    onUpdate: (Float, Float, Float) -> Unit
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(text = bone.name, style = MaterialTheme.typography.titleMedium)
            Spacer(modifier = Modifier.height(8.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("X", modifier = Modifier.width(20.dp))
                Slider(
                    value = bone.pitch,
                    onValueChange = { onUpdate(it, bone.yaw, bone.roll) },
                    valueRange = -90f..90f,
                    modifier = Modifier
                        .weight(1f)
                        .semantics { contentDescription = "${bone.name} X axis pitch" }
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Y", modifier = Modifier.width(20.dp))
                Slider(
                    value = bone.yaw,
                    onValueChange = { onUpdate(bone.pitch, it, bone.roll) },
                    valueRange = -90f..90f,
                    modifier = Modifier
                        .weight(1f)
                        .semantics { contentDescription = "${bone.name} Y axis yaw" }
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Z", modifier = Modifier.width(20.dp))
                Slider(
                    value = bone.roll,
                    onValueChange = { onUpdate(bone.pitch, bone.yaw, it) },
                    valueRange = -90f..90f,
                    modifier = Modifier
                        .weight(1f)
                        .semantics { contentDescription = "${bone.name} Z axis roll" }
                )
            }
        }
    }
}
