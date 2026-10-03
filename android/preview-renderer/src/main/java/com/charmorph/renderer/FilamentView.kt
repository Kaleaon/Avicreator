package com.charmorph.renderer

import android.content.Context
import android.net.Uri
import android.util.AttributeSet
import android.view.SurfaceView
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Skeleton
import com.charmorph.core.model.Vector4
import com.google.android.filament.utils.Utils

class FilamentView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : SurfaceView(context, attrs, defStyleAttr) {

    private var controller: FilamentController? = null
    private var currentMesh: Mesh? = null
    private var showAnatomicalDetails: Boolean = false
    private val currentWeights = mutableMapOf<Int, Float>()

    init {
        Utils.init()
        controller = FilamentController(context, this)
    }

    fun destroy() {
        controller?.destroy()
        controller = null
    }

    fun loadMesh(mesh: Mesh, skeleton: Skeleton? = null) {
        currentMesh = mesh
        controller?.loadMesh(mesh, skeleton)
        updateVisibility()
    }

    fun loadTexture(uri: Uri, type: TextureType) {
        controller?.loadTexture(uri, type)
    }

    fun setAnatomicalDetailsVisible(visible: Boolean) {
        if (showAnatomicalDetails != visible) {
            showAnatomicalDetails = visible
            updateVisibility()
        }
    }

    fun updateMorphWeight(targetName: String, weight: Float) {
        val morphId = targetName.hashCode()
        currentWeights[morphId] = weight
        controller?.updateMorphWeights(currentWeights)
    }

    fun updateBoneRotation(boneId: Int, rotation: Vector4) {
        controller?.updateBoneRotation(boneId, rotation)
    }

    private fun updateVisibility() {
        val mesh = currentMesh ?: return
        mesh.groups.forEach { group ->
             val isSensitive = group.tags.contains("anatomical") || group.tags.contains("genitalia")
             val shouldRender = !isSensitive || showAnatomicalDetails
             controller?.setGroupVisibility(group.name, shouldRender)
        }
    }
}
