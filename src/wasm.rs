#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;
#[cfg(feature = "wasm")]
use glam::Vec3;
#[cfg(feature = "wasm")]
use crate::{
    morpher::{BaseMesh, MorphTarget, Morpher, SparseDelta},
    spatial::BVHTree,
};
#[cfg(feature = "wasm")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "wasm")]
#[derive(Serialize, Deserialize)]
pub struct WasmRayHit {
    pub hit: bool,
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub face_index: u32,
    pub distance: f32,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn raycast_mesh(
    positions: &[f32],
    polys_flat: &[u32],
    origin_x: f32,
    origin_y: f32,
    origin_z: f32,
    dir_x: f32,
    dir_y: f32,
    dir_z: f32,
) -> Option<Vec<f32>> {
    let mut verts = Vec::with_capacity(positions.len() / 3);
    for chunk in positions.chunks_exact(3) {
        verts.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
    }
    let mut polys = Vec::with_capacity(polys_flat.len() / 3);
    for chunk in polys_flat.chunks_exact(3) {
        polys.push([chunk[0] as usize, chunk[1] as usize, chunk[2] as usize]);
    }
    let bvh = BVHTree::FromPolygons(&verts, &polys);
    let hit = bvh.ray_cast(
        Vec3::new(origin_x, origin_y, origin_z),
        Vec3::new(dir_x, dir_y, dir_z),
        f32::INFINITY,
    )?;
    Some(vec![
        hit.point.x, hit.point.y, hit.point.z,
        hit.normal.x, hit.normal.y, hit.normal.z,
        hit.face_index as f32,
        hit.distance,
    ])
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct WasmBVHTree {
    inner: BVHTree,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl WasmBVHTree {
    #[wasm_bindgen(constructor)]
    pub fn new(verts_flat: &[f32], polys_flat: &[u32]) -> Self {
        let mut verts = Vec::with_capacity(verts_flat.len() / 3);
        for chunk in verts_flat.chunks_exact(3) {
            verts.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
        }

        let mut polys = Vec::with_capacity(polys_flat.len() / 3);
        for chunk in polys_flat.chunks_exact(3) {
            polys.push([chunk[0] as usize, chunk[1] as usize, chunk[2] as usize]);
        }

        Self {
            inner: BVHTree::FromPolygons(&verts, &polys),
        }
    }

    pub fn ray_cast(&self, origin_x: f32, origin_y: f32, origin_z: f32, dir_x: f32, dir_y: f32, dir_z: f32, max_dist: f32) -> Option<Vec<f32>> {
        let hit = self.inner.ray_cast(
            Vec3::new(origin_x, origin_y, origin_z),
            Vec3::new(dir_x, dir_y, dir_z),
            max_dist,
        )?;
        Some(vec![
            hit.point.x, hit.point.y, hit.point.z,
            hit.normal.x, hit.normal.y, hit.normal.z,
            hit.face_index as f32,
            hit.distance,
        ])
    }
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct WasmMorpher {
    inner: Morpher,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl WasmMorpher {
    #[wasm_bindgen(constructor)]
    pub fn new(verts_flat: &[f32]) -> Self {
        let mut verts = Vec::with_capacity(verts_flat.len() / 3);
        for chunk in verts_flat.chunks_exact(3) {
            verts.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
        }
        let base_mesh = BaseMesh::new(verts, Vec::new(), Vec::new());
        Self {
            inner: Morpher::new(base_mesh),
        }
    }

    pub fn add_morph_sparse(&mut self, name: &str, min_val: f32, max_val: f32, default_val: f32, indices: &[u32], offsets_flat: &[f32]) {
        let mut deltas = Vec::with_capacity(indices.len());
        for (idx, chunk) in indices.iter().zip(offsets_flat.chunks_exact(3)) {
            deltas.push(SparseDelta {
                vertex_index: *idx as usize,
                offset: Vec3::new(chunk[0], chunk[1], chunk[2]),
            });
        }
        let target = MorphTarget::new_sparse(name, min_val, max_val, default_val, deltas);
        self.inner.add_morph(target);
    }

    pub fn set_weight(&mut self, name: &str, weight: f32) {
        self.inner.set_weight(name, weight);
    }

    pub fn evaluate(&self) -> Vec<f32> {
        let evaluated = self.inner.evaluate();
        let mut flat = Vec::with_capacity(evaluated.len() * 3);
        for v in evaluated {
            flat.push(v.x);
            flat.push(v.y);
            flat.push(v.z);
        }
        flat
    }
}
