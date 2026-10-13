use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MorphTargetData {
    pub name: String,
    pub positions: Vec<Vec3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MeshData {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<[usize; 3]>,
    #[serde(default)]
    pub joints: Vec<[u16; 4]>,
    #[serde(default)]
    pub weights: Vec<[f32; 4]>,
    #[serde(default)]
    pub joint_names: Vec<String>,
    #[serde(default)]
    pub targets: Vec<MorphTargetData>,
}

pub struct GltfIO;

impl GltfIO {
    pub fn import_gltf_bytes(bytes: &[f32], indices: &[usize]) -> MeshData {
        let mut positions = Vec::with_capacity(bytes.len() / 3);
        for chunk in bytes.chunks_exact(3) {
            positions.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
        }

        let mut poly_indices = Vec::with_capacity(indices.len() / 3);
        for chunk in indices.chunks_exact(3) {
            poly_indices.push([chunk[0], chunk[1], chunk[2]]);
        }

        MeshData {
            positions,
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: poly_indices,
            joints: Vec::new(),
            weights: Vec::new(),
            joint_names: Vec::new(),
            targets: Vec::new(),
        }
    }

    pub fn export_gltf_json(mesh: &MeshData) -> String {
        let num_verts = mesh.positions.len();
        let num_indices = mesh.indices.len() * 3;

        let mut accessors = Vec::new();
        let mut buffer_views = Vec::new();
        let mut current_offset = 0usize;

        let align_offset = |offset: usize| -> usize {
            let padding = (4 - (offset % 4)) % 4;
            offset + padding
        };

        // 1. POSITION Accessor (0)
        current_offset = align_offset(current_offset);
        let pos_min = if mesh.positions.is_empty() {
            vec![0.0, 0.0, 0.0]
        } else {
            let pmin = mesh.positions.iter().fold([f32::INFINITY; 3], |acc, v| {
                [acc[0].min(v.x), acc[1].min(v.y), acc[2].min(v.z)]
            });
            vec![pmin[0], pmin[1], pmin[2]]
        };

        let pos_max = if mesh.positions.is_empty() {
            vec![0.0, 0.0, 0.0]
        } else {
            let pmax = mesh.positions.iter().fold([f32::NEG_INFINITY; 3], |acc, v| {
                [acc[0].max(v.x), acc[1].max(v.y), acc[2].max(v.z)]
            });
            vec![pmax[0], pmax[1], pmax[2]]
        };

        let pos_bv_idx = buffer_views.len();
        let pos_len = num_verts * 12;
        buffer_views.push(serde_json::json!({
            "buffer": 0,
            "byteOffset": current_offset,
            "byteLength": pos_len
        }));
        let pos_acc_idx = accessors.len();
        accessors.push(serde_json::json!({
            "bufferView": pos_bv_idx,
            "componentType": 5126, // FLOAT
            "count": num_verts,
            "type": "VEC3",
            "min": pos_min,
            "max": pos_max
        }));
        current_offset += pos_len;

        // 2. INDICES Accessor (1)
        current_offset = align_offset(current_offset);
        let idx_bv_idx = buffer_views.len();
        let idx_len = num_indices * 4;
        buffer_views.push(serde_json::json!({
            "buffer": 0,
            "byteOffset": current_offset,
            "byteLength": idx_len
        }));
        let idx_acc_idx = accessors.len();
        accessors.push(serde_json::json!({
            "bufferView": idx_bv_idx,
            "componentType": 5125, // UNSIGNED_INT
            "count": num_indices,
            "type": "SCALAR"
        }));
        current_offset += idx_len;

        let mut attributes = serde_json::json!({
            "POSITION": pos_acc_idx
        });

        let mut nodes = vec![serde_json::json!({ "name": "CharacterMesh", "mesh": 0 })];
        let mut scene_nodes = vec![0];
        let mut skins = Vec::new();

        // 3. JOINTS_0 & WEIGHTS_0 Accessors (Skinning)
        if !mesh.joints.is_empty() && !mesh.weights.is_empty() {
            // JOINTS_0
            current_offset = align_offset(current_offset);
            let joints_bv_idx = buffer_views.len();
            let joints_len = num_verts * 8; // 4 * u16 = 8 bytes
            buffer_views.push(serde_json::json!({
                "buffer": 0,
                "byteOffset": current_offset,
                "byteLength": joints_len
            }));
            let joints_acc_idx = accessors.len();
            accessors.push(serde_json::json!({
                "bufferView": joints_bv_idx,
                "componentType": 5123, // UNSIGNED_SHORT
                "count": num_verts,
                "type": "VEC4"
            }));
            current_offset += joints_len;

            // WEIGHTS_0
            current_offset = align_offset(current_offset);
            let weights_bv_idx = buffer_views.len();
            let weights_len = num_verts * 16; // 4 * f32 = 16 bytes
            buffer_views.push(serde_json::json!({
                "buffer": 0,
                "byteOffset": current_offset,
                "byteLength": weights_len
            }));
            let weights_acc_idx = accessors.len();
            accessors.push(serde_json::json!({
                "bufferView": weights_bv_idx,
                "componentType": 5126, // FLOAT
                "count": num_verts,
                "type": "VEC4"
            }));
            current_offset += weights_len;

            attributes["JOINTS_0"] = serde_json::json!(joints_acc_idx);
            attributes["WEIGHTS_0"] = serde_json::json!(weights_acc_idx);

            // Create joint nodes & skin hierarchy
            let joint_names = if mesh.joint_names.is_empty() {
                vec!["joint_0".to_string()]
            } else {
                mesh.joint_names.clone()
            };

            let num_joints = joint_names.len();
            let mut joint_node_indices = Vec::new();
            for j_name in &joint_names {
                let node_idx = nodes.len();
                joint_node_indices.push(node_idx);
                nodes.push(serde_json::json!({ "name": j_name }));
            }

            if let Some(&root_joint) = joint_node_indices.first() {
                scene_nodes.push(root_joint);
            }

            // Inverse bind matrices
            current_offset = align_offset(current_offset);
            let inv_bind_bv_idx = buffer_views.len();
            let inv_bind_len = num_joints * 64; // 16 * f32 = 64 bytes
            buffer_views.push(serde_json::json!({
                "buffer": 0,
                "byteOffset": current_offset,
                "byteLength": inv_bind_len
            }));
            let inv_bind_acc_idx = accessors.len();
            accessors.push(serde_json::json!({
                "bufferView": inv_bind_bv_idx,
                "componentType": 5126, // FLOAT
                "count": num_joints,
                "type": "MAT4"
            }));
            current_offset += inv_bind_len;

            skins.push(serde_json::json!({
                "name": "ArmatureSkin",
                "inverseBindMatrices": inv_bind_acc_idx,
                "joints": joint_node_indices
            }));

            nodes[0]["skin"] = serde_json::json!(0);
        }

        // 4. MORPH TARGETS Accessors
        let mut primitive_targets = Vec::new();
        let mut target_names = Vec::new();

        for target in &mesh.targets {
            let t_num = target.positions.len();
            let t_min = if target.positions.is_empty() {
                vec![0.0, 0.0, 0.0]
            } else {
                let pmin = target.positions.iter().fold([f32::INFINITY; 3], |acc, v| {
                    [acc[0].min(v.x), acc[1].min(v.y), acc[2].min(v.z)]
                });
                vec![pmin[0], pmin[1], pmin[2]]
            };

            let t_max = if target.positions.is_empty() {
                vec![0.0, 0.0, 0.0]
            } else {
                let pmax = target.positions.iter().fold([f32::NEG_INFINITY; 3], |acc, v| {
                    [acc[0].max(v.x), acc[1].max(v.y), acc[2].max(v.z)]
                });
                vec![pmax[0], pmax[1], pmax[2]]
            };

            current_offset = align_offset(current_offset);
            let t_bv_idx = buffer_views.len();
            let t_len = t_num * 12;

            buffer_views.push(serde_json::json!({
                "buffer": 0,
                "byteOffset": current_offset,
                "byteLength": t_len
            }));
            let t_acc_idx = accessors.len();
            accessors.push(serde_json::json!({
                "bufferView": t_bv_idx,
                "componentType": 5126, // FLOAT
                "count": t_num,
                "type": "VEC3",
                "min": t_min,
                "max": t_max
            }));
            current_offset += t_len;

            primitive_targets.push(serde_json::json!({
                "POSITION": t_acc_idx
            }));
            target_names.push(target.name.clone());
        }

        let mut primitive = serde_json::json!({
            "attributes": attributes,
            "indices": idx_acc_idx
        });

        if !primitive_targets.is_empty() {
            primitive["targets"] = serde_json::json!(primitive_targets);
        }

        let mut mesh_json = serde_json::json!({
            "name": "AvicreatorMesh",
            "primitives": [primitive]
        });

        if !target_names.is_empty() {
            mesh_json["extras"] = serde_json::json!({
                "targetNames": target_names
            });
            mesh_json["weights"] = serde_json::json!(vec![0.0; target_names.len()]);
        }

        let mut gltf_json = serde_json::json!({
            "asset": { "version": "2.0", "generator": "avicreator-core" },
            "scene": 0,
            "scenes": [{ "nodes": scene_nodes }],
            "nodes": nodes,
            "meshes": [mesh_json],
            "accessors": accessors,
            "bufferViews": buffer_views,
            "buffers": [{ "byteLength": current_offset }]
        });

        if !skins.is_empty() {
            gltf_json["skins"] = serde_json::json!(skins);
        }

        gltf_json.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_gltf_json_skinning_and_morphs() {
        let mesh = MeshData {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            normals: vec![],
            uvs: vec![],
            indices: vec![[0, 1, 2]],
            joints: vec![[0, 0, 0, 0]; 3],
            weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
            joint_names: vec!["root_bone".to_string()],
            targets: vec![MorphTargetData {
                name: "MouthSmile".to_string(),
                positions: vec![
                    Vec3::new(0.1, 0.0, 0.0),
                    Vec3::new(0.0, 0.1, 0.0),
                    Vec3::new(0.0, 0.0, 0.1),
                ],
            }],
        };

        let json_str = GltfIO::export_gltf_json(&mesh);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        let prim = &parsed["meshes"][0]["primitives"][0];
        assert!(prim["attributes"]["POSITION"].is_number());
        assert!(prim["attributes"]["JOINTS_0"].is_number());
        assert!(prim["attributes"]["WEIGHTS_0"].is_number());

        assert!(prim["targets"].is_array());
        assert_eq!(prim["targets"].as_array().unwrap().len(), 1);
        assert!(prim["targets"][0]["POSITION"].is_number());

        let target_names = &parsed["meshes"][0]["extras"]["targetNames"];
        assert_eq!(target_names[0], "MouthSmile");

        assert!(parsed["skins"].is_array());
        assert_eq!(parsed["skins"][0]["name"], "ArmatureSkin");
    }
}
