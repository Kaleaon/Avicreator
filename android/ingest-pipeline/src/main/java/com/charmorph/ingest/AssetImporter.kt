package com.charmorph.ingest

import android.net.Uri
import com.charmorph.core.model.Resource
import com.charmorph.core.model.Mesh
import java.io.File

interface AssetImporter {
    suspend fun importFromUri(uri: Uri, outputDir: File): Resource<Mesh>
    suspend fun extractFeatures(mesh: Mesh): Resource<Map<String, Any>>
}
