package com.charmorph.ingest

import com.charmorph.core.model.Mesh
import com.charmorph.core.model.MeshGroup
import com.charmorph.core.model.Vector2
import com.charmorph.core.model.Vector3
import java.io.BufferedReader
import java.io.InputStream
import java.io.InputStreamReader

object ObjParser {

    fun parse(inputStream: InputStream, name: String): Mesh {
        val vertices = mutableListOf<Vector3>()
        val normals = mutableListOf<Vector3>()
        val uvs = mutableListOf<Vector2>()

        // For simple parsing, we assume the mesh is triangulated and indices refer to v/vt/vn
        // However, Mesh object requires unified indices. This requires re-indexing.

        val indices = mutableListOf<Int>()

        // Group tracking
        val groups = mutableListOf<MeshGroup>()
        var currentGroupName = "default"
        var currentGroupIndices = mutableListOf<Int>()
        var currentTags = mutableListOf<String>()

        // Mapping unique vertex definitions "v/vt/vn" to new index
        val vertexMap = mutableMapOf<String, Int>()
        val finalVertices = mutableListOf<Vector3>()
        val finalNormals = mutableListOf<Vector3>()
        val finalUvs = mutableListOf<Vector2>()

        val reader = BufferedReader(InputStreamReader(inputStream))

        reader.forEachLine { line ->
            if (line.startsWith("#") || line.isBlank()) return@forEachLine

            val parts = line.trim().split("\\s+".toRegex())
            when (parts[0]) {
                "v" -> vertices.add(Vector3(parts[1].toFloat(), parts[2].toFloat(), parts[3].toFloat()))
                "vn" -> normals.add(Vector3(parts[1].toFloat(), parts[2].toFloat(), parts[3].toFloat()))
                "vt" -> uvs.add(Vector2(parts[1].toFloat(), parts[2].toFloat()))
                "g", "o" -> {
                    if (currentGroupIndices.isNotEmpty()) {
                        groups.add(MeshGroup(currentGroupName, currentGroupIndices.toList(), 0, currentTags.toList()))
                        currentGroupIndices.clear()
                        currentTags.clear()
                    }
                    if (parts.size > 1) {
                        currentGroupName = parts[1]
                        // Simple tag heuristic based on name
                        if (currentGroupName.contains("genital", ignoreCase = true)) {
                            currentTags.add("genitalia")
                            currentTags.add("anatomical")
                        }
                    }
                }
                "f" -> {
                    // Assume triangles: f v1/vt1/vn1 v2/vt2/vn2 v3/vt3/vn3
                    for (i in 1..3) {
                        val faceKey = parts[i]
                        val index = vertexMap.getOrPut(faceKey) {
                            val vIdxs = faceKey.split("/")
                            val vI = vIdxs[0].toInt() - 1

                            // Add vertex position
                            finalVertices.add(vertices[vI])

                            // Add UV (if present)
                            if (vIdxs.size > 1 && vIdxs[1].isNotEmpty()) {
                                finalUvs.add(uvs[vIdxs[1].toInt() - 1])
                            } else {
                                finalUvs.add(Vector2(0f, 0f))
                            }

                            // Add Normal (if present)
                            if (vIdxs.size > 2 && vIdxs[2].isNotEmpty()) {
                                finalNormals.add(normals[vIdxs[2].toInt() - 1])
                            } else {
                                finalNormals.add(Vector3(0f, 1f, 0f))
                            }

                            finalVertices.size - 1
                        }
                        indices.add(index)
                        currentGroupIndices.add(index)
                    }
                }
            }
        }

        if (currentGroupIndices.isNotEmpty()) {
            groups.add(MeshGroup(currentGroupName, currentGroupIndices.toList(), 0, currentTags.toList()))
        }

        // If no UVs/Normals were found, fill with defaults to match vertex count
        if (finalNormals.size < finalVertices.size) {
            val diff = finalVertices.size - finalNormals.size
            for(i in 0 until diff) finalNormals.add(Vector3(0f,1f,0f))
        }
        if (finalUvs.size < finalVertices.size) {
            val diff = finalVertices.size - finalUvs.size
             for(i in 0 until diff) finalUvs.add(Vector2(0f,0f))
        }

        return Mesh(
            id = java.util.UUID.randomUUID().toString(),
            name = name,
            vertices = finalVertices,
            normals = finalNormals,
            uvs = finalUvs,
            indices = indices,
            groups = groups
        )
    }
}
