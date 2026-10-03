package com.charmorph.renderer

import android.content.Context
import android.net.Uri
import android.view.Choreographer
import android.view.SurfaceHolder
import android.view.SurfaceView
import com.charmorph.core.model.Mesh
import com.charmorph.core.model.Skeleton
import com.charmorph.core.model.Vector4
import com.charmorph.nativebridge.NativeLib
import com.google.android.filament.Camera
import com.google.android.filament.Engine
import com.google.android.filament.EntityManager
import com.google.android.filament.IndexBuffer
import com.google.android.filament.LightManager
import com.google.android.filament.Material
import com.google.android.filament.MaterialInstance
import com.google.android.filament.RenderableManager
import com.google.android.filament.Renderer
import com.google.android.filament.Scene
import com.google.android.filament.Skybox
import com.google.android.filament.SwapChain
import com.google.android.filament.Texture
import com.google.android.filament.VertexBuffer
import com.google.android.filament.View
import com.google.android.filament.Viewport
import com.google.android.filament.utils.Manipulator
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.nio.ByteBuffer
import java.nio.ByteOrder

class FilamentController(
    private val context: Context,
    private val surfaceView: SurfaceView
) : Choreographer.FrameCallback {

    private var engine: Engine = Engine.create()
    private var renderer: Renderer = engine.createRenderer()
    private var scene: Scene = engine.createScene()
    private var view: View = engine.createView()
    private var camera: Camera = engine.createCamera(engine.entityManager.create())
    private var swapChain: SwapChain? = null
    private var choreographer: Choreographer = Choreographer.getInstance()

    private val entityMap = mutableMapOf<String, Int>()
    private val bufferMap = mutableMapOf<String, Pair<VertexBuffer, IndexBuffer>>()
    private var cameraManipulator: Manipulator? = null

    private var nativeMeshPtr: Long = 0
    private val nativeLib = NativeLib()
    private var skeletonRig: SkeletonRig? = null

    // Materials
    private var pbrMaterial: Material? = null
    private val materialInstances = mutableMapOf<String, MaterialInstance>()
    private var albedoTexture: Texture? = null
    private var normalTexture: Texture? = null

    init {
        view.scene = scene
        view.camera = camera
        setupLighting()
        setupManipulator()
        setupMaterial()

        surfaceView.holder.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {
                swapChain = engine.createSwapChain(holder.surface)
                choreographer.postFrameCallback(this@FilamentController)
            }

            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                view.viewport = Viewport(0, 0, width, height)
                val aspect = width.toDouble() / height.toDouble()
                camera.setProjection(45.0, aspect, 0.1, 20.0, Camera.Fov.VERTICAL)
                cameraManipulator?.setViewport(width, height)
            }

            override fun surfaceDestroyed(holder: SurfaceHolder) {
                choreographer.removeFrameCallback(this@FilamentController)
                swapChain?.let { engine.destroySwapChain(it) }
                swapChain = null
            }
        })
    }

    private fun setupMaterial() {
        pbrMaterial = MaterialFactory.createPbrMaterial(engine)
    }

    fun loadTexture(uri: Uri, type: TextureType) {
        CoroutineScope(Dispatchers.Main).launch {
            val texture = TextureUtils.loadTextureFromUri(context, engine, uri, type == TextureType.ALBEDO)
            if (texture != null) {
                when (type) {
                    TextureType.ALBEDO -> {
                        engine.destroyTexture(albedoTexture)
                        albedoTexture = texture
                        updateMaterialParameters()
                    }
                    TextureType.NORMAL -> {
                        engine.destroyTexture(normalTexture)
                        normalTexture = texture
                        updateMaterialParameters()
                    }
                    else -> {}
                }
            }
        }
    }

    private fun updateMaterialParameters() {
        materialInstances.values.forEach { instance ->
            albedoTexture?.let {
                instance.setParameter("baseColorMap", it, com.google.android.filament.TextureSampler())
            }
            normalTexture?.let {
                instance.setParameter("normalMap", it, com.google.android.filament.TextureSampler())
            }
        }
    }

    private fun setupLighting() {
        scene.skybox = Skybox.Builder().color(0.1f, 0.1f, 0.1f, 1.0f).build(engine)
        val light = EntityManager.get().create()
        LightManager.Builder(LightManager.Type.DIRECTIONAL)
            .color(1.0f, 0.95f, 0.9f)
            .intensity(100000.0f)
            .direction(1.0f, -0.5f, -1.0f)
            .build(engine, light)
        scene.addEntity(light)
    }

    private fun setupManipulator() {
        cameraManipulator = Manipulator.Builder()
            .targetPosition(0.0f, 1.0f, 0.0f)
            .build(Manipulator.Mode.ORBIT)
    }

    fun loadMesh(mesh: Mesh, skeleton: Skeleton? = null) {
        cleanup()

        if (skeleton != null) {
            skeletonRig = SkeletonRig(skeleton)
        } else {
            skeletonRig = null
        }

        val flatVertices = FloatArray(mesh.vertices.size * 3)
        mesh.vertices.forEachIndexed { i, v ->
            flatVertices[i*3] = v.x
            flatVertices[i*3+1] = v.y
            flatVertices[i*3+2] = v.z
        }
        nativeMeshPtr = nativeLib.createMesh(flatVertices)

        val vertexCount = mesh.vertices.size
        val vertexBufferData = ByteBuffer.allocateDirect(vertexCount * 3 * 4)
            .order(ByteOrder.nativeOrder())
        vertexBufferData.asFloatBuffer().put(flatVertices)

        // Flatten UVs for Material support
        val uvData = ByteBuffer.allocateDirect(vertexCount * 2 * 4).order(ByteOrder.nativeOrder())
        mesh.uvs.forEach { uv ->
            uvData.putFloat(uv.u)
            uvData.putFloat(1.0f - uv.v) // Flip V if needed for OpenGL/Filament convention
        }
        uvData.flip()

        val vbBuilder = VertexBuffer.Builder()
            .bufferCount(2) // 0: Position, 1: UV
            .vertexCount(vertexCount)
            .attribute(VertexBuffer.VertexAttribute.POSITION, 0, VertexBuffer.AttributeType.FLOAT3, 0, 12)
            .attribute(VertexBuffer.VertexAttribute.UV0, 1, VertexBuffer.AttributeType.FLOAT2, 0, 8)

        val vb = vbBuilder.build(engine)
        vb.setBufferAt(engine, 0, vertexBufferData)
        vb.setBufferAt(engine, 1, uvData)

        if (mesh.groups.isEmpty()) {
            createEntityForGroup("root", mesh.indices, emptyList(), vb, skeletonRig != null)
        } else {
            mesh.groups.forEach { group ->
                createEntityForGroup(group.name, group.indices, group.tags, vb, skeletonRig != null)
            }
        }

        skeletonRig?.let { updateSkinning(it) }
    }

    private fun createEntityForGroup(name: String, indices: List<Int>, tags: List<String>, vb: VertexBuffer, hasSkinning: Boolean) {
        val indexCount = indices.size
        val indexBufferData = ByteBuffer.allocateDirect(indexCount * 4)
            .order(ByteOrder.nativeOrder())
        indices.forEach { indexBufferData.putInt(it) }
        indexBufferData.flip()

        val ib = IndexBuffer.Builder()
            .indexCount(indexCount)
            .bufferType(IndexBuffer.Builder.IndexType.UINT)
            .build(engine)
        ib.setBuffer(engine, indexBufferData)

        bufferMap[name] = Pair(vb, ib)

        // Create Material Instance for this group
        val matInstance = pbrMaterial?.createInstance()
        if (matInstance != null) {
            matInstance.setParameter("baseColorFactor", 1.0f, 0.8f, 0.6f, 1.0f) // Default Skin Tone
            matInstance.setParameter("roughnessFactor", 0.4f)
            materialInstances[name] = matInstance

            // Apply existing textures
            updateMaterialParameters()
        }

        val entity = EntityManager.get().create()
        val builder = RenderableManager.Builder(1)
            .boundingBox(com.google.android.filament.Box(-2f, -2f, -2f, 2f, 2f, 2f))
            .geometry(0, RenderableManager.PrimitiveType.TRIANGLES, vb, ib)
            .culling(false)

        if (matInstance != null) {
            builder.material(0, matInstance)
        }

        if (hasSkinning) {
            builder.skinning(skeletonRig!!.skinningBuffer.size / 16)
        }

        builder.build(engine, entity)

        scene.addEntity(entity)
        entityMap[name] = entity
    }

    fun updateMorphWeights(weights: Map<Int, Float>) {
        if (nativeMeshPtr == 0L || bufferMap.isEmpty()) return

        val vertexBuffer = bufferMap.values.first().first
        val vertexCount = vertexBuffer.vertexCount

        val outputBuffer = ByteBuffer.allocateDirect(vertexCount * 3 * 4).order(ByteOrder.nativeOrder())

        val ids = weights.keys.toIntArray()
        val values = weights.values.toFloatArray()

        nativeLib.updateMorphs(nativeMeshPtr, ids, values, outputBuffer)

        vertexBuffer.setBufferAt(engine, 0, outputBuffer)
    }

    fun updateBoneRotation(boneId: Int, rotation: Vector4) {
        skeletonRig?.let { rig ->
            rig.updateBone(boneId, rotation)
            updateSkinning(rig)
        }
    }

    private fun updateSkinning(rig: SkeletonRig) {
        entityMap.values.forEach { entity ->
            val rm = engine.renderableManager
            val instance = rm.getInstance(entity)
             rm.setBones(instance, rig.skinningBuffer, 0, rig.skinningBuffer.size / 16)
        }
    }

    private fun cleanup() {
        if (nativeMeshPtr != 0L) {
            nativeLib.destroyMesh(nativeMeshPtr)
            nativeMeshPtr = 0
        }
        entityMap.values.forEach {
            scene.removeEntity(it)
            engine.destroyEntity(it)
        }
        entityMap.clear()
        materialInstances.values.forEach { engine.destroyMaterialInstance(it) }
        materialInstances.clear()
        bufferMap.values.forEach { (_, ib) -> engine.destroyIndexBuffer(ib) }
        if (bufferMap.isNotEmpty()) {
            engine.destroyVertexBuffer(bufferMap.values.first().first)
        }
        bufferMap.clear()
        skeletonRig = null
    }

    fun setGroupVisibility(name: String, visible: Boolean) {
        val entity = entityMap[name] ?: return
        val rm = engine.renderableManager
        val instance = rm.getInstance(entity)
        if (instance != 0) {
            rm.setLayerMask(instance, 0xff, if (visible) 0xff else 0x00)
        }
    }

    override fun doFrame(frameTimeNanos: Long) {
        choreographer.postFrameCallback(this)
        cameraManipulator?.update(frameTimeNanos.toFloat())
        if (view.viewport.width > 0 && view.viewport.height > 0) {
            if (renderer.beginFrame(swapChain!!, frameTimeNanos)) {
                renderer.render(view)
                renderer.endFrame()
            }
        }
    }

    fun destroy() {
        cleanup()
        choreographer.removeFrameCallback(this)
        engine.destroyRenderer(renderer)
        engine.destroyView(view)
        engine.destroyScene(scene)
        engine.destroyCameraComponent(camera.entity)
        engine.entityManager.destroy(camera.entity)
        pbrMaterial?.let { engine.destroyMaterial(it) }
        albedoTexture?.let { engine.destroyTexture(it) }
        normalTexture?.let { engine.destroyTexture(it) }
        engine.destroy()
    }
}

enum class TextureType {
    ALBEDO, NORMAL, ROUGHNESS
}
