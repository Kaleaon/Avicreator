use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshData {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<[usize; 3]>,
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
        }
    }

    pub fn export_gltf_json(mesh: &MeshData) -> String {
        let num_verts = mesh.positions.len();
        let num_indices = mesh.indices.len() * 3;

        let json = serde_json::json!({
            "asset": { "version": "2.0", "generator": "avicreator-core" },
            "scene": 0,
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "mesh": 0 }],
            "meshes": [{
                "name": "AvicreatorMesh",
                "primitives": [{
                    "attributes": {
                        "POSITION": 0
                    },
                    "indices": 1
                }]
            }],
            "accessors": [
                {
                    "bufferView": 0,
                    "componentType": 5126, // FLOAT
                    "count": num_verts,
                    "type": "VEC3"
                },
                {
                    "bufferView": 1,
                    "componentType": 5125, // UNSIGNED_INT
                    "count": num_indices,
                    "type": "SCALAR"
                }
            ],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": num_verts * 12 },
                { "buffer": 0, "byteOffset": num_verts * 12, "byteLength": num_indices * 4 }
            ]
        });

        json.to_string()
    }
}
