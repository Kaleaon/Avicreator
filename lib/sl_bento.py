# ##### BEGIN GPL LICENSE BLOCK #####
#
#  This program is free software; you can redistribute it and/or
#  modify it under the terms of the GNU General Public License
#  as published by the Free Software Foundation; either version 3
#  of the License, or (at your option) any later version.
#
#  This program is distributed in the hope that it will be useful,
#  but WITHOUT ANY WARRANTY; without even the implied warranty of
#  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#  GNU General Public License for more details.
#
#  You should have received a copy of the GNU General Public License
#  along with this program; if not, write to the Free Software Foundation,
#  Inc., 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301, USA.
#
# ##### END GPL LICENSE BLOCK #####

import logging
try:
    import bpy  # pylint: disable=import-error
    from mathutils import Vector  # pylint: disable=import-error
except ImportError:
    bpy = None
    Vector = None

logger = logging.getLogger(__name__)

# Standard Second Life Bento mBone hierarchy definition
# Format: { bone_name: { "parent": parent_name, "head": (x,y,z), "tail": (x,y,z), "deform": True/False } }
SL_BENTO_BONES = {
    "mPelvis": {"parent": None, "head": (0.0, 0.0, 1.0), "tail": (0.0, 0.0, 1.1), "deform": True},
    "mTorso": {"parent": "mPelvis", "head": (0.0, 0.0, 1.1), "tail": (0.0, 0.0, 1.25), "deform": True},
    "mChest": {"parent": "mTorso", "head": (0.0, 0.0, 1.25), "tail": (0.0, 0.0, 1.45), "deform": True},
    "mNeck": {"parent": "mChest", "head": (0.0, 0.0, 1.45), "tail": (0.0, 0.0, 1.58), "deform": True},
    "mHead": {"parent": "mNeck", "head": (0.0, 0.0, 1.58), "tail": (0.0, 0.0, 1.75), "deform": True},
    "mSkull": {"parent": "mHead", "head": (0.0, 0.0, 1.75), "tail": (0.0, 0.0, 1.85), "deform": True},
    "mEyeLeft": {"parent": "mHead", "head": (0.035, 0.08, 1.68), "tail": (0.035, 0.12, 1.68), "deform": True},
    "mEyeRight": {"parent": "mHead", "head": (-0.035, 0.08, 1.68), "tail": (-0.035, 0.12, 1.68), "deform": True},
    "mFaceRoot": {"parent": "mHead", "head": (0.0, 0.08, 1.62), "tail": (0.0, 0.12, 1.62), "deform": True},
    "mFaceEyeAltLeft": {"parent": "mFaceRoot", "head": (0.035, 0.08, 1.68), "tail": (0.035, 0.10, 1.68), "deform": True},
    "mFaceEyeAltRight": {"parent": "mFaceRoot", "head": (-0.035, 0.08, 1.68), "tail": (-0.035, 0.10, 1.68), "deform": True},
    "mFaceJaw": {"parent": "mFaceRoot", "head": (0.0, 0.04, 1.60), "tail": (0.0, 0.09, 1.56), "deform": True},
    "mFaceLipUpperCenter": {"parent": "mFaceRoot", "head": (0.0, 0.09, 1.62), "tail": (0.0, 0.10, 1.62), "deform": True},
    "mFaceLipLowerCenter": {"parent": "mFaceRoot", "head": (0.0, 0.09, 1.60), "tail": (0.0, 0.10, 1.60), "deform": True},

    # Left Arm & Hand
    "mCollarLeft": {"parent": "mChest", "head": (0.02, 0.0, 1.42), "tail": (0.18, 0.0, 1.42), "deform": True},
    "mShoulderLeft": {"parent": "mCollarLeft", "head": (0.18, 0.0, 1.42), "tail": (0.43, 0.0, 1.42), "deform": True},
    "mElbowLeft": {"parent": "mShoulderLeft", "head": (0.43, 0.0, 1.42), "tail": (0.68, 0.0, 1.42), "deform": True},
    "mWristLeft": {"parent": "mElbowLeft", "head": (0.68, 0.0, 1.42), "tail": (0.76, 0.0, 1.42), "deform": True},
    "mHandLeft": {"parent": "mWristLeft", "head": (0.76, 0.0, 1.42), "tail": (0.84, 0.0, 1.42), "deform": True},

    # Left Bento Fingers
    "mHandThumb1Left": {"parent": "mWristLeft", "head": (0.73, 0.03, 1.40), "tail": (0.76, 0.05, 1.39), "deform": True},
    "mHandThumb2Left": {"parent": "mHandThumb1Left", "head": (0.76, 0.05, 1.39), "tail": (0.78, 0.06, 1.38), "deform": True},
    "mHandThumb3Left": {"parent": "mHandThumb2Left", "head": (0.78, 0.06, 1.38), "tail": (0.80, 0.07, 1.37), "deform": True},
    "mHandIndex1Left": {"parent": "mWristLeft", "head": (0.77, 0.02, 1.43), "tail": (0.81, 0.02, 1.43), "deform": True},
    "mHandIndex2Left": {"parent": "mHandIndex1Left", "head": (0.81, 0.02, 1.43), "tail": (0.84, 0.02, 1.43), "deform": True},
    "mHandIndex3Left": {"parent": "mHandIndex2Left", "head": (0.84, 0.02, 1.43), "tail": (0.86, 0.02, 1.43), "deform": True},
    "mHandMiddle1Left": {"parent": "mWristLeft", "head": (0.77, 0.0, 1.43), "tail": (0.82, 0.0, 1.43), "deform": True},
    "mHandMiddle2Left": {"parent": "mHandMiddle1Left", "head": (0.82, 0.0, 1.43), "tail": (0.85, 0.0, 1.43), "deform": True},
    "mHandMiddle3Left": {"parent": "mHandMiddle2Left", "head": (0.85, 0.0, 1.43), "tail": (0.88, 0.0, 1.43), "deform": True},
    "mHandRing1Left": {"parent": "mWristLeft", "head": (0.77, -0.02, 1.43), "tail": (0.81, -0.02, 1.43), "deform": True},
    "mHandRing2Left": {"parent": "mHandRing1Left", "head": (0.81, -0.02, 1.43), "tail": (0.84, -0.02, 1.43), "deform": True},
    "mHandRing3Left": {"parent": "mHandRing2Left", "head": (0.84, -0.02, 1.43), "tail": (0.86, -0.02, 1.43), "deform": True},
    "mHandPinky1Left": {"parent": "mWristLeft", "head": (0.76, -0.04, 1.42), "tail": (0.79, -0.04, 1.42), "deform": True},
    "mHandPinky2Left": {"parent": "mHandPinky1Left", "head": (0.79, -0.04, 1.42), "tail": (0.82, -0.04, 1.42), "deform": True},
    "mHandPinky3Left": {"parent": "mHandPinky2Left", "head": (0.82, -0.04, 1.42), "tail": (0.84, -0.04, 1.42), "deform": True},

    # Right Arm & Hand
    "mCollarRight": {"parent": "mChest", "head": (-0.02, 0.0, 1.42), "tail": (-0.18, 0.0, 1.42), "deform": True},
    "mShoulderRight": {"parent": "mCollarRight", "head": (-0.18, 0.0, 1.42), "tail": (-0.43, 0.0, 1.42), "deform": True},
    "mElbowRight": {"parent": "mShoulderRight", "head": (-0.43, 0.0, 1.42), "tail": (-0.68, 0.0, 1.42), "deform": True},
    "mWristRight": {"parent": "mElbowRight", "head": (-0.68, 0.0, 1.42), "tail": (-0.76, 0.0, 1.42), "deform": True},
    "mHandRight": {"parent": "mWristRight", "head": (-0.76, 0.0, 1.42), "tail": (-0.84, 0.0, 1.42), "deform": True},

    # Right Bento Fingers
    "mHandThumb1Right": {"parent": "mWristRight", "head": (-0.73, 0.03, 1.40), "tail": (-0.76, 0.05, 1.39), "deform": True},
    "mHandThumb2Right": {"parent": "mHandThumb1Right", "head": (-0.76, 0.05, 1.39), "tail": (-0.78, 0.06, 1.38), "deform": True},
    "mHandThumb3Right": {"parent": "mHandThumb2Right", "head": (-0.78, 0.06, 1.38), "tail": (-0.80, 0.07, 1.37), "deform": True},
    "mHandIndex1Right": {"parent": "mWristRight", "head": (-0.77, 0.02, 1.43), "tail": (-0.81, 0.02, 1.43), "deform": True},
    "mHandIndex2Right": {"parent": "mHandIndex1Right", "head": (-0.81, 0.02, 1.43), "tail": (-0.84, 0.02, 1.43), "deform": True},
    "mHandIndex3Right": {"parent": "mHandIndex2Right", "head": (-0.84, 0.02, 1.43), "tail": (-0.86, 0.02, 1.43), "deform": True},
    "mHandMiddle1Right": {"parent": "mWristRight", "head": (-0.77, 0.0, 1.43), "tail": (-0.82, 0.0, 1.43), "deform": True},
    "mHandMiddle2Right": {"parent": "mHandMiddle1Right", "head": (-0.82, 0.0, 1.43), "tail": (-0.85, 0.0, 1.43), "deform": True},
    "mHandMiddle3Right": {"parent": "mHandMiddle2Right", "head": (-0.85, 0.0, 1.43), "tail": (-0.88, 0.0, 1.43), "deform": True},
    "mHandRing1Right": {"parent": "mWristRight", "head": (-0.77, -0.02, 1.43), "tail": (-0.81, -0.02, 1.43), "deform": True},
    "mHandRing2Right": {"parent": "mHandRing1Right", "head": (-0.81, -0.02, 1.43), "tail": (-0.84, -0.02, 1.43), "deform": True},
    "mHandRing3Right": {"parent": "mHandRing2Right", "head": (-0.84, -0.02, 1.43), "tail": (-0.86, -0.02, 1.43), "deform": True},
    "mHandPinky1Right": {"parent": "mWristRight", "head": (-0.76, -0.04, 1.42), "tail": (-0.79, -0.04, 1.42), "deform": True},
    "mHandPinky2Right": {"parent": "mHandPinky1Right", "head": (-0.79, -0.04, 1.42), "tail": (-0.82, -0.04, 1.42), "deform": True},
    "mHandPinky3Right": {"parent": "mHandPinky2Right", "head": (-0.82, -0.04, 1.42), "tail": (-0.84, -0.04, 1.42), "deform": True},

    # Legs & Feet
    "mHipLeft": {"parent": "mPelvis", "head": (0.10, 0.0, 1.0), "tail": (0.10, 0.0, 0.52), "deform": True},
    "mKneeLeft": {"parent": "mHipLeft", "head": (0.10, 0.0, 0.52), "tail": (0.10, 0.0, 0.08), "deform": True},
    "mAnkleLeft": {"parent": "mKneeLeft", "head": (0.10, 0.0, 0.08), "tail": (0.10, 0.12, 0.0), "deform": True},
    "mFootLeft": {"parent": "mAnkleLeft", "head": (0.10, 0.12, 0.0), "tail": (0.10, 0.18, 0.0), "deform": True},
    "mToeLeft": {"parent": "mFootLeft", "head": (0.10, 0.18, 0.0), "tail": (0.10, 0.22, 0.0), "deform": True},

    "mHipRight": {"parent": "mPelvis", "head": (-0.10, 0.0, 1.0), "tail": (-0.10, 0.0, 0.52), "deform": True},
    "mKneeRight": {"parent": "mHipRight", "head": (-0.10, 0.0, 0.52), "tail": (-0.10, 0.0, 0.08), "deform": True},
    "mAnkleRight": {"parent": "mKneeRight", "head": (-0.10, 0.0, 0.08), "tail": (-0.10, 0.12, 0.0), "deform": True},
    "mFootRight": {"parent": "mAnkleRight", "head": (-0.10, 0.12, 0.0), "tail": (-0.10, 0.18, 0.0), "deform": True},
    "mToeRight": {"parent": "mFootRight", "head": (-0.10, 0.18, 0.0), "tail": (-0.10, 0.22, 0.0), "deform": True},
}

# Collision volume bone definitions (cBones)
SL_COLLISION_VOLUMES = {
    "PELVIS": {"parent": "mPelvis", "head": (0.0, 0.0, 1.0), "tail": (0.0, 0.0, 1.05), "deform": False},
    "TORSO": {"parent": "mTorso", "head": (0.0, 0.0, 1.1), "tail": (0.0, 0.0, 1.15), "deform": False},
    "CHEST": {"parent": "mChest", "head": (0.0, 0.0, 1.25), "tail": (0.0, 0.0, 1.30), "deform": False},
    "NECK": {"parent": "mNeck", "head": (0.0, 0.0, 1.45), "tail": (0.0, 0.0, 1.50), "deform": False},
    "HEAD": {"parent": "mHead", "head": (0.0, 0.0, 1.58), "tail": (0.0, 0.0, 1.65), "deform": False},
    "HIP_LEFT": {"parent": "mHipLeft", "head": (0.10, 0.0, 0.95), "tail": (0.10, 0.0, 0.85), "deform": False},
    "HIP_RIGHT": {"parent": "mHipRight", "head": (-0.10, 0.0, 0.95), "tail": (-0.10, 0.0, 0.85), "deform": False},
    "KNEELFT": {"parent": "mKneeLeft", "head": (0.10, 0.0, 0.52), "tail": (0.10, 0.0, 0.45), "deform": False},
    "KNEERHT": {"parent": "mKneeRight", "head": (-0.10, 0.0, 0.52), "tail": (-0.10, 0.0, 0.45), "deform": False},
    "ANKLELFT": {"parent": "mAnkleLeft", "head": (0.10, 0.0, 0.08), "tail": (0.10, 0.05, 0.05), "deform": False},
    "ANKLERHT": {"parent": "mAnkleRight", "head": (-0.10, 0.0, 0.08), "tail": (-0.10, 0.05, 0.05), "deform": False},
    "FOOTLFT": {"parent": "mFootLeft", "head": (0.10, 0.12, 0.0), "tail": (0.10, 0.15, 0.0), "deform": False},
    "FOOTRHT": {"parent": "mFootRight", "head": (-0.10, 0.12, 0.0), "tail": (-0.10, 0.15, 0.0), "deform": False},
    "SHOULDERLFT": {"parent": "mShoulderLeft", "head": (0.18, 0.0, 1.42), "tail": (0.25, 0.0, 1.42), "deform": False},
    "SHOULDERRHT": {"parent": "mShoulderRight", "head": (-0.18, 0.0, 1.42), "tail": (-0.25, 0.0, 1.42), "deform": False},
    "ELBOWLFT": {"parent": "mElbowLeft", "head": (0.43, 0.0, 1.42), "tail": (0.50, 0.0, 1.42), "deform": False},
    "ELBOWRHT": {"parent": "mElbowRight", "head": (-0.43, 0.0, 1.42), "tail": (-0.50, 0.0, 1.42), "deform": False},
    "WRISTLFT": {"parent": "mWristLeft", "head": (0.68, 0.0, 1.42), "tail": (0.72, 0.0, 1.42), "deform": False},
    "WRISTRHT": {"parent": "mWristRight", "head": (-0.68, 0.0, 1.42), "tail": (-0.72, 0.0, 1.42), "deform": False},
    "BUTT": {"parent": "mPelvis", "head": (0.0, -0.08, 0.95), "tail": (0.0, -0.12, 0.95), "deform": False},
    "BELLY": {"parent": "mTorso", "head": (0.0, 0.08, 1.15), "tail": (0.0, 0.12, 1.15), "deform": False},
    "L_PEC": {"parent": "mChest", "head": (0.08, 0.08, 1.30), "tail": (0.08, 0.12, 1.30), "deform": False},
    "R_PEC": {"parent": "mChest", "head": (-0.08, 0.08, 1.30), "tail": (-0.08, 0.12, 1.30), "deform": False},
    "UPPER_ARM_LEFT": {"parent": "mShoulderLeft", "head": (0.30, 0.0, 1.42), "tail": (0.35, 0.0, 1.42), "deform": False},
    "UPPER_ARM_RIGHT": {"parent": "mShoulderRight", "head": (-0.30, 0.0, 1.42), "tail": (-0.35, 0.0, 1.42), "deform": False},
    "LOWER_ARM_LEFT": {"parent": "mElbowLeft", "head": (0.55, 0.0, 1.42), "tail": (0.60, 0.0, 1.42), "deform": False},
    "LOWER_ARM_RIGHT": {"parent": "mElbowRight", "head": (-0.55, 0.0, 1.42), "tail": (-0.60, 0.0, 1.42), "deform": False},
    "THIGH_LEFT": {"parent": "mHipLeft", "head": (0.10, 0.0, 0.75), "tail": (0.10, 0.0, 0.65), "deform": False},
    "THIGH_RIGHT": {"parent": "mHipRight", "head": (-0.10, 0.0, 0.75), "tail": (-0.10, 0.0, 0.65), "deform": False},
    "SHIN_LEFT": {"parent": "mKneeLeft", "head": (0.10, 0.0, 0.30), "tail": (0.10, 0.0, 0.20), "deform": False},
    "SHIN_RIGHT": {"parent": "mKneeRight", "head": (-0.10, 0.0, 0.30), "tail": (-0.10, 0.0, 0.20), "deform": False},
    "L_GROIN": {"parent": "mPelvis", "head": (0.05, 0.04, 0.90), "tail": (0.05, 0.06, 0.90), "deform": False},
    "R_GROIN": {"parent": "mPelvis", "head": (-0.05, 0.04, 0.90), "tail": (-0.05, 0.06, 0.90), "deform": False},
    "L_CLAVICLE": {"parent": "mCollarLeft", "head": (0.10, 0.0, 1.42), "tail": (0.14, 0.0, 1.42), "deform": False},
    "R_CLAVICLE": {"parent": "mCollarRight", "head": (-0.10, 0.0, 1.42), "tail": (-0.14, 0.0, 1.42), "deform": False},
}

# Mapping table translating CharMorph/MB-Lab vertex groups to SL Bento mBones
CHARMORPH_TO_SL_WEIGHT_MAP = {
    "pelvis": "mPelvis",
    "spine1": "mTorso",
    "spine2": "mTorso",
    "chest": "mChest",
    "neck": "mNeck",
    "head": "mHead",
    "head_eye.L": "mEyeLeft",
    "head_eye.R": "mEyeRight",
    "jaw": "mFaceJaw",
    "thigh.L": "mHipLeft",
    "shin.L": "mKneeLeft",
    "foot.L": "mAnkleLeft",
    "toe.L": "mToeLeft",
    "thigh.R": "mHipRight",
    "shin.R": "mKneeRight",
    "foot.R": "mAnkleRight",
    "toe.R": "mToeRight",
    "shoulder.L": "mCollarLeft",
    "upper_arm.L": "mShoulderLeft",
    "forearm.L": "mElbowLeft",
    "hand.L": "mWristLeft",
    "shoulder.R": "mCollarRight",
    "upper_arm.R": "mShoulderRight",
    "forearm.R": "mElbowRight",
    "hand.R": "mWristRight",
    "thumb.01.L": "mHandThumb1Left",
    "thumb.02.L": "mHandThumb2Left",
    "thumb.03.L": "mHandThumb3Left",
    "f_index.01.L": "mHandIndex1Left",
    "f_index.02.L": "mHandIndex2Left",
    "f_index.03.L": "mHandIndex3Left",
    "f_middle.01.L": "mHandMiddle1Left",
    "f_middle.02.L": "mHandMiddle2Left",
    "f_middle.03.L": "mHandMiddle3Left",
    "f_ring.01.L": "mHandRing1Left",
    "f_ring.02.L": "mHandRing2Left",
    "f_ring.03.L": "mHandRing3Left",
    "f_pinky.01.L": "mHandPinky1Left",
    "f_pinky.02.L": "mHandPinky2Left",
    "f_pinky.03.L": "mHandPinky3Left",
    "thumb.01.R": "mHandThumb1Right",
    "thumb.02.R": "mHandThumb2Right",
    "thumb.03.R": "mHandThumb3Right",
    "f_index.01.R": "mHandIndex1Right",
    "f_index.02.R": "mHandIndex2Right",
    "f_index.03.R": "mHandIndex3Right",
    "f_middle.01.R": "mHandMiddle1Right",
    "f_middle.02.R": "mHandMiddle2Right",
    "f_middle.03.R": "mHandMiddle3Right",
    "f_ring.01.R": "mHandRing1Right",
    "f_ring.02.R": "mHandRing2Right",
    "f_ring.03.R": "mHandRing3Right",
    "f_pinky.01.R": "mHandPinky1Right",
    "f_pinky.02.R": "mHandPinky2Right",
    "f_pinky.03.R": "mHandPinky3Right",
    "breast.L": "L_PEC",
    "breast.R": "R_PEC",
}


def is_sl_armature(obj):
    """
    Validate whether an object is a Second Life compatible armature.
    Checks for key SL Bento mBones (mPelvis, mChest, mHead).
    """
    if not obj:
        return False

    bones = set()
    if hasattr(obj, "type") and obj.type == "ARMATURE" and hasattr(obj, "data") and obj.data:
        bones = set(obj.data.bones.keys())
    elif hasattr(obj, "bones"):
        bones = set(obj.bones.keys())
    elif isinstance(obj, dict) and "bones" in obj:
        bones = set(obj["bones"].keys())

    sl_required = {"mPelvis", "mChest", "mHead"}
    return bool(sl_required.issubset(bones)) or bool(sl_required.intersection(bones))


def build_sl_bento_armature(armature_data_name="sl_bento_armature"):
    """
    Construct a Second Life Bento armature object in Blender.
    """
    if bpy is None:
        return None

    arm_data = bpy.data.armatures.new(armature_data_name)
    rig_obj = bpy.data.objects.new("sl_bento", arm_data)

    if bpy.context and bpy.context.collection:
        bpy.context.collection.objects.link(rig_obj)

    bpy.context.view_layer.objects.active = rig_obj
    bpy.ops.object.mode_set(mode="EDIT")

    edit_bones = arm_data.edit_bones

    # 1. Create mBones
    all_bones = {}
    all_bones.update(SL_BENTO_BONES)
    all_bones.update(SL_COLLISION_VOLUMES)

    for bone_name, info in all_bones.items():
        eb = edit_bones.new(bone_name)
        eb.head = Vector(info["head"])
        eb.tail = Vector(info["tail"])
        eb.use_deform = info.get("deform", True)

    # 2. Set parenting
    for bone_name, info in all_bones.items():
        eb = edit_bones.get(bone_name)
        parent_name = info.get("parent")
        if parent_name and parent_name in edit_bones:
            eb.parent = edit_bones[parent_name]

    bpy.ops.object.mode_set(mode="OBJECT")
    return rig_obj


def transfer_sl_weights(mesh_obj):
    """
    Remap/copy vertex group weights from CharMorph base mesh vertex groups to SL Bento mBones.
    """
    if not mesh_obj or not hasattr(mesh_obj, "vertex_groups"):
        return

    vgs = mesh_obj.vertex_groups
    for cm_group, sl_bone in CHARMORPH_TO_SL_WEIGHT_MAP.items():
        if cm_group in vgs and sl_bone not in vgs:
            src_vg = vgs[cm_group]
            dst_vg = vgs.new(name=sl_bone)
            # Copy weights from src_vg to dst_vg
            for v in mesh_obj.data.vertices:
                for g in v.groups:
                    if g.group == src_vg.index:
                        dst_vg.add([v.index], g.weight, 'REPLACE')


def is_sl_armature_valid(mesh_obj, armature_obj=None):
    """
    Validate that the active character mesh is rigged to an SL-compatible armature (e.g. sl_bento).
    """
    if armature_obj is None and mesh_obj:
        if hasattr(mesh_obj, "find_armature"):
            armature_obj = mesh_obj.find_armature()
        elif hasattr(mesh_obj, "parent") and mesh_obj.parent and getattr(mesh_obj.parent, "type", "") == "ARMATURE":
            armature_obj = mesh_obj.parent

    if not armature_obj:
        return False

    # Check rig type or bone names
    rig_type = None
    if hasattr(armature_obj, "data") and hasattr(armature_obj.data, "get"):
        rig_type = armature_obj.data.get("charmorph_rig_type")
    elif isinstance(armature_obj, dict):
        rig_type = armature_obj.get("charmorph_rig_type")

    if rig_type == "sl_bento":
        return True

    return is_sl_armature(armature_obj)


def generate_collada_dae_xml(filepath, mesh_obj, armature_obj=None):
    """
    Generate Collada .dae XML file formatted for Second Life & OpenSim mesh importers.
    Extracts real vertex positions, normals, triangle indices, bind shape matrices,
    mBone joint hierarchy, and skin weight maps from mesh_obj (BaseMesh, Object, or Dict).
    """
    import xml.etree.ElementTree as ET
    from xml.dom import minidom
    import datetime
    import numpy as np

    try:
        from . import gltf_exporter
    except ImportError:
        import gltf_exporter

    extracted = gltf_exporter.extract_export_mesh_data(mesh_obj, armature_obj)

    positions = extracted["positions"]
    normals = extracted["normals"]
    triangles = extracted["triangle_indices"]
    joints = extracted["joint_indices"]
    weights = extracted["skin_weights"]
    joint_names = extracted["joint_names"]
    bones_dict = extracted["bones_dict"]

    mesh_name = getattr(mesh_obj, "name", "character_mesh")
    if isinstance(mesh_obj, dict) and "name" in mesh_obj:
        mesh_name = mesh_obj["name"]

    num_verts = len(positions)
    num_tris = len(triangles) // 3 if len(triangles) >= 3 else 0

    collada = ET.Element("COLLADA", {
        "xmlns": "http://www.collada.org/2005/11/COLLADASchema",
        "version": "1.4.1"
    })

    # 1. Asset
    asset = ET.SubElement(collada, "asset")
    contributor = ET.SubElement(asset, "contributor")
    authoring_tool = ET.SubElement(contributor, "authoring_tool")
    authoring_tool.text = "CharMorph Second Life Collada Exporter"

    now_str = datetime.datetime.now().strftime("%Y-%m-%dT%H:%M:%SZ")
    created = ET.SubElement(asset, "created")
    created.text = now_str
    modified = ET.SubElement(asset, "modified")
    modified.text = now_str

    unit = ET.SubElement(asset, "unit", {"name": "meter", "meter": "1.0"})
    up_axis = ET.SubElement(asset, "up_axis")
    up_axis.text = "Z_UP"

    # 2. Library Geometries
    lib_geom = ET.SubElement(collada, "library_geometries")
    geom = ET.SubElement(lib_geom, "geometry", {"id": f"{mesh_name}-mesh", "name": mesh_name})
    mesh_elem = ET.SubElement(geom, "mesh")

    # Positions source
    pos_str = " ".join(f"{x:.6f} {y:.6f} {z:.6f}" for x, y, z in positions)
    pos_src = ET.SubElement(mesh_elem, "source", {"id": f"{mesh_name}-mesh-positions"})
    pos_array = ET.SubElement(pos_src, "float_array", {"id": f"{mesh_name}-mesh-positions-array", "count": str(num_verts * 3)})
    pos_array.text = pos_str
    tech_pos = ET.SubElement(pos_src, "technique_common")
    acc_pos = ET.SubElement(tech_pos, "accessor", {"source": f"#{mesh_name}-mesh-positions-array", "count": str(num_verts), "stride": "3"})
    for axis in ["X", "Y", "Z"]:
        ET.SubElement(acc_pos, "param", {"name": axis, "type": "float"})

    # Normals source
    norm_str = " ".join(f"{x:.6f} {y:.6f} {z:.6f}" for x, y, z in normals)
    norm_src = ET.SubElement(mesh_elem, "source", {"id": f"{mesh_name}-mesh-normals"})
    norm_array = ET.SubElement(norm_src, "float_array", {"id": f"{mesh_name}-mesh-normals-array", "count": str(num_verts * 3)})
    norm_array.text = norm_str
    tech_norm = ET.SubElement(norm_src, "technique_common")
    acc_norm = ET.SubElement(tech_norm, "accessor", {"source": f"#{mesh_name}-mesh-normals-array", "count": str(num_verts), "stride": "3"})
    for axis in ["X", "Y", "Z"]:
        ET.SubElement(acc_norm, "param", {"name": axis, "type": "float"})

    vertices = ET.SubElement(mesh_elem, "vertices", {"id": f"{mesh_name}-mesh-vertices"})
    ET.SubElement(vertices, "input", {"semantic": "POSITION", "source": f"#{mesh_name}-mesh-positions"})

    triangles_elem = ET.SubElement(mesh_elem, "triangles", {"count": str(num_tris)})
    ET.SubElement(triangles_elem, "input", {"semantic": "VERTEX", "source": f"#{mesh_name}-mesh-vertices", "offset": "0"})
    ET.SubElement(triangles_elem, "input", {"semantic": "NORMAL", "source": f"#{mesh_name}-mesh-normals", "offset": "1"})

    p_indices = []
    for idx in triangles:
        p_indices.extend([str(idx), str(idx)])
    p_elem = ET.SubElement(triangles_elem, "p")
    p_elem.text = " ".join(p_indices)

    # 3. Library Controllers
    lib_ctrl = ET.SubElement(collada, "library_controllers")
    ctrl = ET.SubElement(lib_ctrl, "controller", {"id": f"Armature_{mesh_name}-skin", "name": "Armature"})
    skin = ET.SubElement(ctrl, "skin", {"source": f"#{mesh_name}-mesh"})

    bind_shape = ET.SubElement(skin, "bind_shape_matrix")
    bind_shape.text = "1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1"

    num_joints = len(joint_names)
    # Joint Name Array
    joints_src = ET.SubElement(skin, "source", {"id": f"Armature_{mesh_name}-skin-joints"})
    j_array = ET.SubElement(joints_src, "Name_array", {"id": f"Armature_{mesh_name}-skin-joints-array", "count": str(num_joints)})
    j_array.text = " ".join(joint_names)
    j_tech = ET.SubElement(joints_src, "technique_common")
    j_acc = ET.SubElement(j_tech, "accessor", {"source": f"#Armature_{mesh_name}-skin-joints-array", "count": str(num_joints), "stride": "1"})
    ET.SubElement(j_acc, "param", {"name": "JOINT", "type": "Name"})

    # Inverse Bind Poses Array
    inv_bind_matrices = []
    for j_name in joint_names:
        head = (0.0, 0.0, 0.0)
        if isinstance(bones_dict, dict) and j_name in bones_dict:
            b = bones_dict[j_name]
            head = b.head if hasattr(b, "head") else b.get("head", (0.0, 0.0, 0.0))
        elif j_name in SL_BENTO_BONES:
            head = SL_BENTO_BONES[j_name]["head"]
        # Inverse bind matrix (row-major for Collada: 1 0 0 -hx  0 1 0 -hy  0 0 1 -hz  0 0 0 1)
        inv_matrix_str = f"1 0 0 {-head[0]:.6f} 0 1 0 {-head[1]:.6f} 0 0 1 {-head[2]:.6f} 0 0 0 1"
        inv_bind_matrices.append(inv_matrix_str)

    poses_src = ET.SubElement(skin, "source", {"id": f"Armature_{mesh_name}-skin-bind_poses"})
    p_array = ET.SubElement(poses_src, "float_array", {"id": f"Armature_{mesh_name}-skin-bind_poses-array", "count": str(num_joints * 16)})
    p_array.text = " ".join(inv_bind_matrices)
    p_tech = ET.SubElement(poses_src, "technique_common")
    p_acc = ET.SubElement(p_tech, "accessor", {"source": f"#Armature_{mesh_name}-skin-bind_poses-array", "count": str(num_joints), "stride": "16"})
    ET.SubElement(p_acc, "param", {"name": "TRANSFORM", "type": "float4x4"})

    # Weights Array
    # Flatten weights per vertex into unique list
    weight_list = []
    vcount_list = []
    v_pairs = []

    for vidx in range(num_verts):
        v_joints = joints[vidx]
        v_weights = weights[vidx]
        v_active = [(j, w) for j, w in zip(v_joints, v_weights) if w > 0.0]
        if not v_active and num_joints > 0:
            v_active = [(0, 1.0)]

        vcount_list.append(str(len(v_active)))
        for j_idx, w_val in v_active:
            w_idx = len(weight_list)
            weight_list.append(f"{w_val:.6f}")
            v_pairs.extend([str(j_idx), str(w_idx)])

    weights_src = ET.SubElement(skin, "source", {"id": f"Armature_{mesh_name}-skin-weights"})
    w_array = ET.SubElement(weights_src, "float_array", {"id": f"Armature_{mesh_name}-skin-weights-array", "count": str(len(weight_list))})
    w_array.text = " ".join(weight_list)
    w_tech = ET.SubElement(weights_src, "technique_common")
    w_acc = ET.SubElement(w_tech, "accessor", {"source": f"#Armature_{mesh_name}-skin-weights-array", "count": str(len(weight_list)), "stride": "1"})
    ET.SubElement(w_acc, "param", {"name": "WEIGHT", "type": "float"})

    # Joints Element
    joints_elem = ET.SubElement(skin, "joints")
    ET.SubElement(joints_elem, "input", {"semantic": "JOINT", "source": f"#Armature_{mesh_name}-skin-joints"})
    ET.SubElement(joints_elem, "input", {"semantic": "INV_BIND_MATRIX", "source": f"#Armature_{mesh_name}-skin-bind_poses"})

    # Vertex Weights Element
    vw_elem = ET.SubElement(skin, "vertex_weights", {"count": str(num_verts)})
    ET.SubElement(vw_elem, "input", {"semantic": "JOINT", "source": f"#Armature_{mesh_name}-skin-joints", "offset": "0"})
    ET.SubElement(vw_elem, "input", {"semantic": "WEIGHT", "source": f"#Armature_{mesh_name}-skin-weights", "offset": "1"})

    vcount_elem = ET.SubElement(vw_elem, "vcount")
    vcount_elem.text = " ".join(vcount_list)
    v_elem = ET.SubElement(vw_elem, "v")
    v_elem.text = " ".join(v_pairs)

    # 4. Library Visual Scenes
    lib_scenes = ET.SubElement(collada, "library_visual_scenes")
    scene = ET.SubElement(lib_scenes, "visual_scene", {"id": "Scene", "name": "Scene"})

    # Build Joint Hierarchy
    joint_nodes = {}
    root_joint_name = joint_names[0] if joint_names else "mPelvis"

    for j_name in joint_names:
        parent_name = None
        head = (0.0, 0.0, 0.0)
        if isinstance(bones_dict, dict) and j_name in bones_dict:
            b = bones_dict[j_name]
            head = b.head if hasattr(b, "head") else b.get("head", (0.0, 0.0, 0.0))
            parent_name = b.parent if hasattr(b, "parent") else b.get("parent")
        elif j_name in SL_BENTO_BONES:
            head = SL_BENTO_BONES[j_name]["head"]
            parent_name = SL_BENTO_BONES[j_name].get("parent")

        parent_node = joint_nodes.get(parent_name, scene) if parent_name else scene
        j_node = ET.SubElement(parent_node, "node", {"id": j_name, "name": j_name, "type": "JOINT"})

        # Translate offset relative to parent
        p_head = (0.0, 0.0, 0.0)
        if parent_name:
            if isinstance(bones_dict, dict) and parent_name in bones_dict:
                pb = bones_dict[parent_name]
                p_head = pb.head if hasattr(pb, "head") else pb.get("head", (0.0, 0.0, 0.0))
            elif parent_name in SL_BENTO_BONES:
                p_head = SL_BENTO_BONES[parent_name]["head"]

        rel_t = (head[0] - p_head[0], head[1] - p_head[1], head[2] - p_head[2])
        trans = ET.SubElement(j_node, "translate")
        trans.text = f"{rel_t[0]:.6f} {rel_t[1]:.6f} {rel_t[2]:.6f}"

        joint_nodes[j_name] = j_node

    mesh_node = ET.SubElement(scene, "node", {"id": mesh_name, "name": mesh_name, "type": "NODE"})
    inst_ctrl = ET.SubElement(mesh_node, "instance_controller", {"url": f"#Armature_{mesh_name}-skin"})
    skel = ET.SubElement(inst_ctrl, "skeleton")
    skel.text = f"#{root_joint_name}"

    # Scene
    scene_elem = ET.SubElement(collada, "scene")
    ET.SubElement(scene_elem, "instance_visual_scene", {"url": "#Scene"})

    xml_str = minidom.parseString(ET.tostring(collada)).toprettyxml(indent="  ")
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(xml_str)


def export_collada_sl(filepath, mesh_obj, armature_obj=None):
    """
    Export character mesh and armature to Collada (.dae) format configured for Second Life / OpenSim compatibility.
    """
    if not is_sl_armature_valid(mesh_obj, armature_obj):
        raise ValueError("Active character mesh is not rigged to a Second Life compatible armature (sl_bento)")

    if bpy is not None and hasattr(bpy.ops, "wm") and hasattr(bpy.ops.wm, "collada_export"):
        try:
            # Deselect all
            if hasattr(bpy.ops.object, "select_all"):
                bpy.ops.object.select_all(action='DESELECT')

            if mesh_obj:
                mesh_obj.select_set(True)
            if armature_obj:
                armature_obj.select_set(True)

            bpy.ops.wm.collada_export(
                filepath=filepath,
                check_existing=False,
                export_global_up_selection='Z',
                use_texture_copies=False,
                apply_modifiers=True
            )
            return
        except Exception as e:
            logger.warning("bpy.ops.wm.collada_export failed: %s, falling back to direct Collada generator", str(e))

    generate_collada_dae_xml(filepath, mesh_obj, armature_obj)
