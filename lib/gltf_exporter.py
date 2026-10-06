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

"""
Pure Python glTF 2.0 Exporter Engine.
Generates compliant .glb (Binary glTF 2.0) and .gltf + .bin character assets
without relying on Blender window manager operators or host runtime bindings.
"""

import json
import os
import struct
import numpy as np

try:
    from . import sl_bento, xml_base_mesh, utils
except ImportError:
    import sl_bento, xml_base_mesh, utils


def _compute_normals(positions: np.ndarray, triangle_indices: np.ndarray) -> np.ndarray:
    """Compute per-vertex unit normals from triangle indices."""
    num_verts = len(positions)
    normals = np.zeros((num_verts, 3), dtype=np.float32)

    if len(triangle_indices) == 0:
        return normals

    tris = triangle_indices.reshape(-1, 3)
    p0 = positions[tris[:, 0]]
    p1 = positions[tris[:, 1]]
    p2 = positions[tris[:, 2]]

    v10 = p1 - p0
    v20 = p2 - p0
    face_normals = np.cross(v10, v20)

    # Accumulate face normals into vertices
    for i in range(3):
        np.add.at(normals, tris[:, i], face_normals)

    # Normalize vertex normals
    norms = np.linalg.norm(normals, axis=1, keepdims=True)
    norms[norms == 0] = 1.0
    normals = normals / norms
    return normals.astype(np.float32)


def extract_export_mesh_data(mesh_obj_or_data, armature_obj_or_data=None):
    """
    Extract normalized vertex arrays (positions, normals, uvs, joint_indices, skin_weights)
    and joint hierarchy from mesh/armature objects (BaseMesh, Blender Object, or Dict).
    """
    positions = None
    faces = None
    normals = None
    uvs = None
    bones_dict = {}
    weight_layers = {}

    # 1. BaseMesh / XMLBaseMesh instance
    if isinstance(mesh_obj_or_data, xml_base_mesh.BaseMesh):
        bm = mesh_obj_or_data
        positions = bm.vertex_array
        faces = bm.faces
        bones_dict = bm.bones
        weight_layers = bm.weight_layers
    # 2. Morpher or Character object wrapping xml_base_mesh or obj
    elif hasattr(mesh_obj_or_data, "char") and hasattr(mesh_obj_or_data.char, "xml_base_mesh") and mesh_obj_or_data.char.xml_base_mesh:
        bm = mesh_obj_or_data.char.xml_base_mesh
        positions = bm.vertex_array
        faces = bm.faces
        bones_dict = bm.bones
        weight_layers = bm.weight_layers
    # 3. Blender Object or MockObject
    elif hasattr(mesh_obj_or_data, "data") and mesh_obj_or_data.data is not None:
        obj = mesh_obj_or_data
        data = obj.data

        # Try morphed coordinates first
        if hasattr(utils, "get_morphed_numpy"):
            try:
                positions = utils.get_morphed_numpy(obj)
            except Exception:
                positions = None

        if positions is None and hasattr(data, "vertices"):
            if hasattr(utils, "verts_to_numpy"):
                positions = utils.verts_to_numpy(data.vertices)
            else:
                positions = np.array([v.co for v in data.vertices], dtype=np.float32)

        if hasattr(data, "polygons"):
            faces = [tuple(p.vertices) for p in data.polygons]

        if hasattr(data, "vertices") and hasattr(data.vertices[0], "normal"):
            normals = np.array([v.normal for v in data.vertices], dtype=np.float32)

        # Handle vertex group weights
        if hasattr(obj, "vertex_groups") and hasattr(data, "vertices"):
            vgs = {vg.index: vg.name for vg in obj.vertex_groups}
            bone_weights = {}
            for v in data.vertices:
                if hasattr(v, "groups"):
                    for g in v.groups:
                        g_name = vgs.get(g.group, str(g.group))
                        if g_name not in bone_weights:
                            bone_weights[g_name] = np.zeros(len(data.vertices), dtype=np.float32)
                        bone_weights[g_name][v.index] = g.weight
            if bone_weights:
                weight_layers["skin"] = xml_base_mesh.WeightLayer(
                    name="skin", layer_type="skin", normalised=True, weights={}
                )
                weight_layers["skin"]._dense_weights = bone_weights
    # 4. Dictionary
    elif isinstance(mesh_obj_or_data, dict):
        d = mesh_obj_or_data
        positions = np.asarray(d.get("vertices") or d.get("positions"), dtype=np.float32)
        faces = d.get("faces") or d.get("triangles") or d.get("indices")
        normals = np.asarray(d.get("normals"), dtype=np.float32) if "normals" in d else None
        uvs = np.asarray(d.get("uvs"), dtype=np.float32) if "uvs" in d else None
        if "bones" in d:
            bones_dict = d["bones"]

    if positions is None:
        positions = np.array([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], dtype=np.float32)
        faces = [(0, 1, 2), (0, 2, 3)]

    num_verts = len(positions)

    # Triangulate faces
    triangles = []
    if faces is not None:
        for face in faces:
            if len(face) < 3:
                continue
            v0 = face[0]
            for i in range(1, len(face) - 1):
                triangles.append((v0, face[i], face[i + 1]))
    triangle_indices = np.array(triangles, dtype=np.uint32) if triangles else np.arange(num_verts, dtype=np.uint32)

    # Normals
    if normals is None or len(normals) != num_verts:
        normals = _compute_normals(positions, triangle_indices)

    # UVs
    if uvs is None or len(uvs) != num_verts:
        uvs = np.zeros((num_verts, 2), dtype=np.float32)

    # Joints & Weights
    # Determine joint hierarchy & weight map
    dense_weights = {}
    if weight_layers:
        skin_layer = weight_layers.get("skin") or next(iter(weight_layers.values()))
        if hasattr(skin_layer, "_dense_weights"):
            dense_weights = skin_layer._dense_weights
        else:
            dense_weights = skin_layer.as_normalized_numpy(num_verts)

    # Collect joint list
    if bones_dict:
        if isinstance(bones_dict, dict):
            joint_names = list(bones_dict.keys())
        elif isinstance(bones_dict, list):
            joint_names = [b.name if hasattr(b, "name") else str(b) for b in bones_dict]
        else:
            joint_names = []
    else:
        joint_names = list(dense_weights.keys()) if dense_weights else list(sl_bento.SL_BENTO_BONES.keys())

    joint_to_idx = {name: idx for idx, name in enumerate(joint_names)}

    joint_indices = np.zeros((num_verts, 4), dtype=np.uint16)
    skin_weights = np.zeros((num_verts, 4), dtype=np.float32)

    if dense_weights:
        for vidx in range(num_verts):
            influences = []
            for b_name, w_array in dense_weights.items():
                w = float(w_array[vidx]) if vidx < len(w_array) else 0.0
                if w > 0.0:
                    # Map to SL joint name if mapped
                    sl_name = sl_bento.CHARMORPH_TO_SL_WEIGHT_MAP.get(b_name, b_name)
                    j_idx = joint_to_idx.get(sl_name, joint_to_idx.get(b_name))
                    if j_idx is not None:
                        influences.append((j_idx, w))

            influences.sort(key=lambda x: x[1], reverse=True)
            top4 = influences[:4]
            total_w = sum(w for _, w in top4)
            for i, (j_idx, w) in enumerate(top4):
                joint_indices[vidx, i] = j_idx
                skin_weights[vidx, i] = (w / total_w) if total_w > 0 else 0.0

            if total_w == 0.0 and len(joint_names) > 0:
                joint_indices[vidx, 0] = 0
                skin_weights[vidx, 0] = 1.0

    return {
        "positions": positions.astype(np.float32),
        "normals": normals.astype(np.float32),
        "uvs": uvs.astype(np.float32),
        "triangle_indices": triangle_indices.astype(np.uint32),
        "joint_indices": joint_indices.astype(np.uint16),
        "skin_weights": skin_weights.astype(np.float32),
        "joint_names": joint_names,
        "bones_dict": bones_dict,
    }


def export_gltf_standalone(filepath, mesh_data, armature_data=None, format="glb"):
    """
    Export mesh and armature data to glTF 2.0 (.glb or .gltf + .bin) without calling Blender APIs.
    """
    extracted = extract_export_mesh_data(mesh_data, armature_data)

    positions = extracted["positions"]
    normals = extracted["normals"]
    uvs = extracted["uvs"]
    indices = extracted["triangle_indices"]
    joints = extracted["joint_indices"]
    weights = extracted["skin_weights"]
    joint_names = extracted["joint_names"]
    bones_dict = extracted["bones_dict"]

    num_verts = len(positions)
    num_indices = len(indices)
    num_joints = len(joint_names)

    buffer_bytes = bytearray()
    buffer_views = []
    accessors = []

    def _add_buffer_view(data_bytes, target=None):
        # 4-byte alignment requirement
        offset = len(buffer_bytes)
        padding = (4 - (offset % 4)) % 4
        if padding:
            buffer_bytes.extend(b'\x00' * padding)
            offset += padding

        bv_idx = len(buffer_views)
        bv = {
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": len(data_bytes)
        }
        if target is not None:
            bv["target"] = target
        buffer_views.append(bv)
        buffer_bytes.extend(data_bytes)
        return bv_idx

    def _add_accessor(bv_idx, component_type, type_str, count, min_val=None, max_val=None):
        acc_idx = len(accessors)
        acc = {
            "bufferView": bv_idx,
            "byteOffset": 0,
            "componentType": component_type,
            "count": count,
            "type": type_str
        }
        if min_val is not None:
            acc["min"] = min_val
        if max_val is not None:
            acc["max"] = max_val
        accessors.append(acc)
        return acc_idx

    # 1. Indices Accessor
    # Use UNSIGNED_SHORT if max index fits in uint16, else UNSIGNED_INT
    if num_verts < 65535:
        indices_arr = indices.astype(np.uint16)
        idx_comp_type = 5123  # UNSIGNED_SHORT
    else:
        indices_arr = indices.astype(np.uint32)
        idx_comp_type = 5125  # UNSIGNED_INT

    bv_idx = _add_buffer_view(indices_arr.tobytes(), target=34963)  # ELEMENT_ARRAY_BUFFER
    acc_indices = _add_accessor(bv_idx, idx_comp_type, "SCALAR", num_indices)

    # 2. Position Accessor (Required min/max)
    pos_min = [float(x) for x in np.min(positions, axis=0)]
    pos_max = [float(x) for x in np.max(positions, axis=0)]
    bv_pos = _add_buffer_view(positions.tobytes(), target=34962)  # ARRAY_BUFFER
    acc_pos = _add_accessor(bv_pos, 5126, "VEC3", num_verts, min_val=pos_min, max_val=pos_max)

    # 3. Normal Accessor
    bv_norm = _add_buffer_view(normals.tobytes(), target=34962)
    acc_norm = _add_accessor(bv_norm, 5126, "VEC3", num_verts)

    # 4. Texcoord Accessor
    bv_uv = _add_buffer_view(uvs.tobytes(), target=34962)
    acc_uv = _add_accessor(bv_uv, 5126, "VEC2", num_verts)

    # 5. Joints Accessor
    bv_joints = _add_buffer_view(joints.tobytes(), target=34962)
    acc_joints = _add_accessor(bv_joints, 5123, "VEC4", num_verts)  # UNSIGNED_SHORT

    # 6. Weights Accessor
    bv_weights = _add_buffer_view(weights.tobytes(), target=34962)
    acc_weights = _add_accessor(bv_weights, 5126, "VEC4", num_verts)  # FLOAT

    # Build Nodes & Skin
    nodes = []
    # Node 0: Mesh Node
    mesh_node = {
        "name": "CharacterMesh",
        "mesh": 0
    }
    if num_joints > 0:
        mesh_node["skin"] = 0
    nodes.append(mesh_node)

    # Joint nodes
    joint_node_indices = []
    inv_bind_matrices = []

    for j_idx, j_name in enumerate(joint_names):
        node_idx = len(nodes)
        joint_node_indices.append(node_idx)

        # Get head position
        head = (0.0, 0.0, 0.0)
        parent_name = None
        if isinstance(bones_dict, dict) and j_name in bones_dict:
            b = bones_dict[j_name]
            head = b.head if hasattr(b, "head") else b.get("head", (0.0, 0.0, 0.0))
            parent_name = b.parent if hasattr(b, "parent") else b.get("parent")
        elif j_name in sl_bento.SL_BENTO_BONES:
            b = sl_bento.SL_BENTO_BONES[j_name]
            head = b["head"]
            parent_name = b.get("parent")

        node_dict = {
            "name": j_name,
            "translation": [float(head[0]), float(head[1]), float(head[2])]
        }
        nodes.append(node_dict)

        # Compute Inverse Bind Matrix (4x4 column-major)
        # Translation inverse = [-hx, -hy, -hz]
        inv_matrix = np.eye(4, dtype=np.float32)
        inv_matrix[3, 0] = -float(head[0])
        inv_matrix[3, 1] = -float(head[1])
        inv_matrix[3, 2] = -float(head[2])
        inv_bind_matrices.append(inv_matrix)

    # Inverse Bind Matrices Accessor
    acc_inv_bind = None
    if num_joints > 0:
        inv_bind_arr = np.array(inv_bind_matrices, dtype=np.float32).reshape(-1, 4, 4)
        bv_inv = _add_buffer_view(inv_bind_arr.tobytes(), target=None)  # No target for matrices
        acc_inv_bind = _add_accessor(bv_inv, 5126, "MAT4", num_joints)

    # Parent-child linking for joints
    scene_nodes = [0]  # Mesh node
    if joint_node_indices:
        scene_nodes.append(joint_node_indices[0])  # Root joint node

    skins = []
    if num_joints > 0 and acc_inv_bind is not None:
        skin_dict = {
            "name": "ArmatureSkin",
            "inverseBindMatrices": acc_inv_bind,
            "joints": joint_node_indices
        }
        skins.append(skin_dict)

    # Primary Mesh Primitive
    attributes = {
        "POSITION": acc_pos,
        "NORMAL": acc_norm,
        "TEXCOORD_0": acc_uv,
    }
    if num_joints > 0:
        attributes["JOINTS_0"] = acc_joints
        attributes["WEIGHTS_0"] = acc_weights

    mesh_primitive = {
        "attributes": attributes,
        "indices": acc_indices
    }

    gltf_json = {
        "asset": {
            "version": "2.0",
            "generator": "CharMorph glTF 2.0 Exporter"
        },
        "scene": 0,
        "scenes": [
            {
                "name": "Scene",
                "nodes": scene_nodes
            }
        ],
        "nodes": nodes,
        "meshes": [
            {
                "name": "CharacterMesh",
                "primitives": [mesh_primitive]
            }
        ],
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [
            {
                "byteLength": len(buffer_bytes)
            }
        ]
    }

    if skins:
        gltf_json["skins"] = skins

    # Check format (.glb vs .gltf)
    fmt = format.lower()
    if fmt == "gltf" or filepath.lower().endswith(".gltf"):
        bin_filename = os.path.splitext(os.path.basename(filepath))[0] + ".bin"
        bin_filepath = os.path.splitext(filepath)[0] + ".bin"
        gltf_json["buffers"][0]["uri"] = bin_filename

        with open(bin_filepath, "wb") as f_bin:
            f_bin.write(buffer_bytes)

        with open(filepath, "w", encoding="utf-8") as f_json:
            json.dump(gltf_json, f_json, indent=2)
    else:  # Binary GLB format
        # Align JSON and BIN chunk lengths to 4-byte boundaries
        json_str = json.dumps(gltf_json, separators=(',', ':'))
        json_bytes = json_str.encode('utf-8')
        json_pad = (4 - (len(json_bytes) % 4)) % 4
        json_bytes += b' ' * json_pad

        bin_pad = (4 - (len(buffer_bytes) % 4)) % 4
        buffer_bytes.extend(b'\x00' * bin_pad)

        total_length = 12 + 8 + len(json_bytes) + 8 + len(buffer_bytes)

        # GLB Header: magic (4B), version (4B), length (4B)
        glb_header = struct.pack('<4sII', b'glTF', 2, total_length)
        # JSON Chunk Header: length (4B), type (4B)
        json_chunk_header = struct.pack('<II', len(json_bytes), 0x4E4F534A)  # 'JSON'
        # BIN Chunk Header: length (4B), type (4B)
        bin_chunk_header = struct.pack('<II', len(buffer_bytes), 0x00414E49)   # 'BIN\x00'

        with open(filepath, "wb") as f_glb:
            f_glb.write(glb_header)
            f_glb.write(json_chunk_header)
            f_glb.write(json_bytes)
            f_glb.write(bin_chunk_header)
            f_glb.write(buffer_bytes)


def export_gltf(filepath, mesh_obj_or_data, armature_obj_or_data=None, format="glb", bake_shape_keys=True):
    """
    Main entry point for glTF export.
    Supports both Blender runtime contexts and standalone Python environments.
    """
    if bake_shape_keys and hasattr(file_io_bake_module(), "bake_shape_keys_for_export"):
        try:
            file_io_bake_module().bake_shape_keys_for_export(mesh_obj_or_data)
        except Exception:
            pass

    export_gltf_standalone(filepath, mesh_obj_or_data, armature_obj_or_data, format)


def file_io_bake_module():
    import sys
    return sys.modules.get("file_io") or sys.modules.get("CharMorphExpansion.file_io") or object()
