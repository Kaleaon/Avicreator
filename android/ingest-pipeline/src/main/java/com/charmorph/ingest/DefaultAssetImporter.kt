package com.charmorph.ingest

import android.content.Context
import android.net.Uri
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Resource
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileInputStream
import java.io.InputStream
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class DefaultAssetImporter @Inject constructor(
    @ApplicationContext private val context: Context
) : AssetImporter {

    override suspend fun importFromUri(uri: Uri, outputDir: File): Resource<Mesh> = withContext(Dispatchers.IO) {
        try {
            val inputStream: InputStream = when (uri.scheme) {
                "file" -> FileInputStream(File(uri.path ?: uri.toString()))
                "content" -> context.contentResolver.openInputStream(uri)
                    ?: return@withContext Resource.Error(IllegalArgumentException("Cannot open content stream for $uri"))
                else -> {
                    val file = File(uri.path ?: uri.toString())
                    if (file.exists()) {
                        FileInputStream(file)
                    } else {
                        context.contentResolver.openInputStream(uri)
                            ?: return@withContext Resource.Error(IllegalArgumentException("Cannot open stream for $uri"))
                    }
                }
            }

            val meshName = uri.lastPathSegment ?: "Imported Mesh"
            val mesh = inputStream.use { stream ->
                ObjParser.parse(stream, meshName)
            }
            Resource.Success(mesh)
        } catch (e: Exception) {
            Resource.Error(e)
        }
    }

    override suspend fun extractFeatures(mesh: Mesh): Resource<Map<String, Any>> = withContext(Dispatchers.IO) {
        try {
            var minX = Float.MAX_VALUE
            var minY = Float.MAX_VALUE
            var minZ = Float.MAX_VALUE
            var maxX = -Float.MAX_VALUE
            var maxY = -Float.MAX_VALUE
            var maxZ = -Float.MAX_VALUE

            mesh.vertices.forEach { v ->
                if (v.x < minX) minX = v.x
                if (v.y < minY) minY = v.y
                if (v.z < minZ) minZ = v.z
                if (v.x > maxX) maxX = v.x
                if (v.y > maxY) maxY = v.y
                if (v.z > maxZ) maxZ = v.z
            }

            val hasVertices = mesh.vertices.isNotEmpty()
            val boundsWidth = if (hasVertices) maxX - minX else 0f
            val boundsHeight = if (hasVertices) maxY - minY else 0f
            val boundsDepth = if (hasVertices) maxZ - minZ else 0f

            val features = mapOf<String, Any>(
                "vertexCount" to mesh.vertices.size,
                "faceCount" to mesh.indices.size / 3,
                "groupCount" to mesh.groups.size,
                "boundsWidth" to boundsWidth,
                "boundsHeight" to boundsHeight,
                "boundsDepth" to boundsDepth,
                "tags" to mesh.groups.flatMap { it.tags }.distinct()
            )

            Resource.Success(features)
        } catch (e: Exception) {
            Resource.Error(e)
        }
    }
}
