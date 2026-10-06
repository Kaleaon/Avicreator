package com.charmorph.ingest

import com.charmorph.core.model.Bone
import com.charmorph.core.model.Skeleton
import com.charmorph.core.model.Vector3
import com.charmorph.core.model.Vector4

object GltfSkeletonStub {

    fun createStandardBiped(): Skeleton {
        // Mimics a simplified game skeleton structure (Root -> Pelvis -> Spine...)
        // IDs are simplified for this stub
        val root = Bone(0, "Root", -1, Vector3(0f,0f,0f), Vector4(0f,0f,0f,1f), Vector3(1f,1f,1f))
        val pelvis = Bone(1, "Pelvis", 0, Vector3(0f,1f,0f), Vector4(0f,0f,0f,1f), Vector3(1f,1f,1f))
        val spine = Bone(2, "Spine", 1, Vector3(0f,0.2f,0f), Vector4(0f,0f,0f,1f), Vector3(1f,1f,1f))
        val head = Bone(3, "Head", 2, Vector3(0f,0.4f,0f), Vector4(0f,0f,0f,1f), Vector3(1f,1f,1f))

        return Skeleton(listOf(root, pelvis, spine, head))
    }
}
