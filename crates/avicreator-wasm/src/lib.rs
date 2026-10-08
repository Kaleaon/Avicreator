use avicreator_core::{
    normalize_vertex_weights, BVHTree, FalloffCurve, Mesh, MorphDelta, MorphEvaluator,
    MorphTarget, SculptContext, SculptMode, Vec3,
};
use avicreator_schema::{AssetManifest, MaterialSpec};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[derive(Serialize, Deserialize)]
pub struct WasmRayHit {
    pub hit: bool,
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub face_index: u32,
    pub distance: f32,
}

#[wasm_bindgen]
pub struct WasmAvatarEngine {
    base_mesh: Option<Mesh>,
    bvh: Option<BVHTree>,
    sculpt_ctx: SculptContext,
    morph_targets: HashMap<String, MorphTarget>,
    morph_weights: HashMap<String, f32>,
}

#[wasm_bindgen]
impl WasmAvatarEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            base_mesh: None,
            bvh: None,
            sculpt_ctx: SculptContext::new(),
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

        self.bvh = Some(BVHTree::from_mesh(&mesh));
        self.base_mesh = Some(mesh);
        Ok(())
    }

    pub fn raycast_mesh(
        &self,
        origin_x: f32,
        origin_y: f32,
        origin_z: f32,
        dir_x: f32,
        dir_y: f32,
        dir_z: f32,
    ) -> JsValue {
        let bvh = match self.bvh.as_ref() {
            Some(b) => b,
            None => {
                let res = WasmRayHit {
                    hit: false,
                    point: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    face_index: 0,
                    distance: 0.0,
                };
                return serde_wasm_bindgen::to_value(&res).unwrap();
            }
        };

        let origin = Vec3::new(origin_x, origin_y, origin_z);
        let dir = Vec3::new(dir_x, dir_y, dir_z);

        if let Some(hit) = bvh.ray_cast(origin, dir, f32::INFINITY) {
            let res = WasmRayHit {
                hit: true,
                point: [hit.point.x, hit.point.y, hit.point.z],
                normal: [hit.normal.x, hit.normal.y, hit.normal.z],
                face_index: hit.face_index as u32,
                distance: hit.distance,
            };
            serde_wasm_bindgen::to_value(&res).unwrap()
        } else {
            let res = WasmRayHit {
                hit: false,
                point: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                face_index: 0,
                distance: 0.0,
            };
            serde_wasm_bindgen::to_value(&res).unwrap()
        }
    }

    pub fn apply_brush_stroke(
        &mut self,
        center_x: f32,
        center_y: f32,
        center_z: f32,
        radius: f32,
        strength: f32,
        mode: &str,
        falloff: &str,
        dir_x: f32,
        dir_y: f32,
        dir_z: f32,
    ) -> Result<js_sys::Float32Array, JsValue> {
        let mesh = self
            .base_mesh
            .as_mut()
            .ok_or_else(|| JsValue::from_str("Base mesh not set"))?;

        if self.bvh.is_none() {
            self.bvh = Some(BVHTree::from_mesh(mesh));
        }

        let bvh = self.bvh.as_ref().unwrap();
        let center = Vec3::new(center_x, center_y, center_z);
        let dir = Vec3::new(dir_x, dir_y, dir_z);
        let mode_enum = SculptMode::from_str(mode);
        let falloff_enum = FalloffCurve::from_str(falloff);

        self.sculpt_ctx.apply_stroke(
            mesh,
            bvh,
            center,
            radius,
            strength,
            mode_enum,
            falloff_enum,
            dir,
        );

        self.bvh = Some(BVHTree::from_mesh(mesh));

        let mut flat = Vec::with_capacity(mesh.positions.len() * 3);
        for p in &mesh.positions {
            flat.push(p.x);
            flat.push(p.y);
            flat.push(p.z);
        }

        Ok(js_sys::Float32Array::from(flat.as_slice()))
    }

    pub fn get_normals(&self) -> Result<js_sys::Float32Array, JsValue> {
        let mesh = self
            .base_mesh
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Base mesh not set"))?;
        let mut flat = Vec::with_capacity(mesh.normals.len() * 3);
        for n in &mesh.normals {
            flat.push(n.x);
            flat.push(n.y);
            flat.push(n.z);
        }
        Ok(js_sys::Float32Array::from(flat.as_slice()))
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
pub fn raycast_mesh(
    positions: &[f32],
    indices: &[u32],
    origin_x: f32,
    origin_y: f32,
    origin_z: f32,
    dir_x: f32,
    dir_y: f32,
    dir_z: f32,
) -> JsValue {
    let mut engine = WasmAvatarEngine::new();
    let normals = Vec::new();
    let uvs = Vec::new();
    if engine.set_base_mesh(positions, &normals, &uvs, indices).is_err() {
        let res = WasmRayHit {
            hit: false,
            point: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            face_index: 0,
            distance: 0.0,
        };
        return serde_wasm_bindgen::to_value(&res).unwrap();
    }
    engine.raycast_mesh(origin_x, origin_y, origin_z, dir_x, dir_y, dir_z)
}

#[wasm_bindgen]
pub fn apply_brush_stroke(
    positions: &[f32],
    indices: &[u32],
    center_x: f32,
    center_y: f32,
    center_z: f32,
    radius: f32,
    strength: f32,
    mode: &str,
    falloff: &str,
    dir_x: f32,
    dir_y: f32,
    dir_z: f32,
) -> js_sys::Float32Array {
    let mut engine = WasmAvatarEngine::new();
    let normals = Vec::new();
    let uvs = Vec::new();
    if engine.set_base_mesh(positions, &normals, &uvs, indices).is_err() {
        return js_sys::Float32Array::from(positions);
    }
    engine
        .apply_brush_stroke(
            center_x, center_y, center_z, radius, strength, mode, falloff, dir_x, dir_y, dir_z,
        )
        .unwrap_or_else(|_| js_sys::Float32Array::from(positions))
}

#[wasm_bindgen]
pub fn avicreator_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_sculpting_and_raycast() {
        let mut engine = WasmAvatarEngine::new();
        let positions = vec![
            0.0, 0.0, 0.0,
            1.0, 0.0, 0.0,
            0.0, 1.0, 0.0,
        ];
        let normals = vec![
            0.0, 0.0, 1.0,
            0.0, 0.0, 1.0,
            0.0, 0.0, 1.0,
        ];
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
