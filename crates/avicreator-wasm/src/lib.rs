use avicreator_core::{
    normalize_vertex_weights, Mesh, MorphDelta, MorphEvaluator, MorphTarget, Vec3,
};
use avicreator_schema::{AssetManifest, MaterialSpec};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmAvatarEngine {
    base_mesh: Option<Mesh>,
    morph_targets: HashMap<String, MorphTarget>,
    morph_weights: HashMap<String, f32>,
}

#[wasm_bindgen]
impl WasmAvatarEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            base_mesh: None,
            morph_targets: HashMap::new(),
            morph_weights: HashMap::new(),
        }
    }

    pub fn set_base_mesh(
        &mut self,
        positions: &[f32],
        normals: &[f32],
        uvs: &[f32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        if positions.len() % 3 != 0 {
            return Err(JsValue::from_str("Positions length must be a multiple of 3"));
        }
        let vertex_count = positions.len() / 3;

        let mut pos_vec = Vec::with_capacity(vertex_count);
        for chunk in positions.chunks_exact(3) {
            pos_vec.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
        }

        let mut norm_vec = Vec::with_capacity(vertex_count);
        if !normals.is_empty() {
            if normals.len() != positions.len() {
                return Err(JsValue::from_str("Normals length must match positions length"));
            }
            for chunk in normals.chunks_exact(3) {
                norm_vec.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
            }
        } else {
            norm_vec.resize(vertex_count, Vec3::Y);
        }

        let mut uv_vec = Vec::with_capacity(vertex_count);
        if !uvs.is_empty() {
            for chunk in uvs.chunks_exact(2) {
                uv_vec.push([chunk[0], chunk[1]]);
            }
        } else {
            uv_vec.resize(vertex_count, [0.0, 0.0]);
        }

        let mesh = Mesh {
            positions: pos_vec,
            normals: norm_vec,
            uvs: uv_vec,
            indices: indices.to_vec(),
            bone_weights: vec![[1.0, 0.0, 0.0, 0.0]; vertex_count],
            bone_indices: vec![[0, 0, 0, 0]; vertex_count],
        };

        self.base_mesh = Some(mesh);
        Ok(())
    }

    pub fn add_morph_target(&mut self, name: &str, delta_positions: &[f32]) -> Result<(), JsValue> {
        if delta_positions.len() % 3 != 0 {
            return Err(JsValue::from_str("Delta positions length must be a multiple of 3"));
        }
        let count = delta_positions.len() / 3;
        let mut deltas = Vec::with_capacity(count);
        for chunk in delta_positions.chunks_exact(3) {
            deltas.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
        }

        let target = MorphTarget {
            name: name.to_string(),
            deltas: MorphDelta {
                position_deltas: deltas,
                normal_deltas: None,
            },
            default_weight: 0.0,
        };

        self.morph_targets.insert(name.to_string(), target);
        Ok(())
    }

    pub fn set_morph_weight(&mut self, name: &str, weight: f32) {
        self.morph_weights.insert(name.to_string(), weight);
    }

    pub fn compute_deformed_mesh(&self) -> Result<js_sys::Float32Array, JsValue> {
        let base = self
            .base_mesh
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Base mesh not set"))?;

        let mut active_morphs = Vec::new();
        for (name, weight) in &self.morph_weights {
            if let Some(target) = self.morph_targets.get(name) {
                active_morphs.push((target, *weight));
            }
        }

        let evaluator = MorphEvaluator::new();
        let deformed = evaluator.evaluate(base, &active_morphs);

        let mut flat = Vec::with_capacity(deformed.positions.len() * 3);
        for p in &deformed.positions {
            flat.push(p.x);
            flat.push(p.y);
            flat.push(p.z);
        }

        Ok(js_sys::Float32Array::from(flat.as_slice()))
    }

    pub fn normalize_weights(&self, weights: &[f32], epsilon: f32) -> Result<js_sys::Float32Array, JsValue> {
        if weights.len() % 4 != 0 {
            return Err(JsValue::from_str("Weights length must be a multiple of 4"));
        }
        let count = weights.len() / 4;
        let mut chunks: Vec<[f32; 4]> = Vec::with_capacity(count);
        for chunk in weights.chunks_exact(4) {
            chunks.push([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }

        normalize_vertex_weights(&mut chunks, epsilon);

        let mut flat = Vec::with_capacity(weights.len());
        for c in chunks {
            flat.push(c[0]);
            flat.push(c[1]);
            flat.push(c[2]);
            flat.push(c[3]);
        }

        Ok(js_sys::Float32Array::from(flat.as_slice()))
    }

    pub fn parse_manifest(&self, json_str: &str) -> Result<JsValue, JsValue> {
        let manifest = AssetManifest::from_json(json_str)
            .map_err(|e| JsValue::from_str(&format!("Invalid manifest JSON: {}", e)))?;
        serde_wasm_bindgen::to_value(&manifest)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    pub fn parse_material(&self, json_str: &str) -> Result<JsValue, JsValue> {
        let mat = MaterialSpec::from_json(json_str)
            .map_err(|e| JsValue::from_str(&format!("Invalid material JSON: {}", e)))?;
        serde_wasm_bindgen::to_value(&mat)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }
}

impl Default for WasmAvatarEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
pub fn avicreator_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_engine_internal_logic() {
        let mut engine = WasmAvatarEngine::new();
        let positions = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let normals = vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let uvs = vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let indices = vec![0, 1, 2];

        engine
            .set_base_mesh(&positions, &normals, &uvs, &indices)
            .unwrap();

        let smile_deltas = vec![0.0, 0.2, 0.0, 0.0, 0.2, 0.0, 0.0, 0.2, 0.0];
        engine.add_morph_target("smile", &smile_deltas).unwrap();
        engine.set_morph_weight("smile", 0.5);

        assert_eq!(avicreator_version(), "0.1.0");
    }
}
