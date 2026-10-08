use crate::spatial::{
    calculate_enclosed_volume, calculate_surface_normals, calculate_volume_and_center,
    calculate_volumetric_center, CotangentLaplacianCache,
};
use glam::Vec3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseDelta {
    pub vertex_index: usize,
    pub offset: Vec3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MorphDelta {
    Sparse(Vec<SparseDelta>),
    Dense(Vec<Vec3>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphTarget {
    pub name: String,
    pub min_value: f32,
    pub max_value: f32,
    pub default_value: f32,
    pub delta: MorphDelta,
}

impl MorphTarget {
    pub fn new_sparse(
        name: impl Into<String>,
        min_val: f32,
        max_val: f32,
        default_val: f32,
        deltas: Vec<SparseDelta>,
    ) -> Self {
        Self {
            name: name.into(),
            min_value: min_val,
            max_value: max_val,
            default_value: default_val,
            delta: MorphDelta::Sparse(deltas),
        }
    }

    pub fn new_dense(
        name: impl Into<String>,
        min_val: f32,
        max_val: f32,
        default_val: f32,
        deltas: Vec<Vec3>,
    ) -> Self {
        Self {
            name: name.into(),
            min_value: min_val,
            max_value: max_val,
            default_value: default_val,
            delta: MorphDelta::Dense(deltas),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseMesh {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub polygons: Vec<[usize; 3]>,
}

impl BaseMesh {
    pub fn new(vertices: Vec<Vec3>, normals: Vec<Vec3>, polygons: Vec<[usize; 3]>) -> Self {
        Self {
            vertices,
            normals,
            polygons,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Morpher {
    pub base_mesh: BaseMesh,
    pub morphs: HashMap<String, MorphTarget>,
    pub weights: HashMap<String, f32>,
    pub clamp: bool,
    pub enable_relaxation: bool,
    pub relaxation_factor: f32,
    pub max_iterations: usize,
    pub convergence_threshold: f32,
    pub laplacian_cache: Option<CotangentLaplacianCache>,
}

impl Morpher {
    pub fn new(base_mesh: BaseMesh) -> Self {
        let laplacian_cache = if !base_mesh.polygons.is_empty() {
            Some(CotangentLaplacianCache::build(
                &base_mesh.vertices,
                &base_mesh.polygons,
            ))
        } else {
            None
        };

        Self {
            base_mesh,
            morphs: HashMap::new(),
            weights: HashMap::new(),
            clamp: true,
            enable_relaxation: true,
            relaxation_factor: 0.5,
            max_iterations: 5,
            convergence_threshold: 1e-4,
            laplacian_cache,
        }
    }

    pub fn set_base_mesh(&mut self, base_mesh: BaseMesh) {
        self.laplacian_cache = if !base_mesh.polygons.is_empty() {
            Some(CotangentLaplacianCache::build(
                &base_mesh.vertices,
                &base_mesh.polygons,
            ))
        } else {
            None
        };
        self.base_mesh = base_mesh;
    }

    pub fn add_morph(&mut self, morph: MorphTarget) {
        let name = morph.name.clone();
        let default_val = morph.default_value;
        self.morphs.insert(name.clone(), morph);
        self.weights.insert(name, default_val);
    }

    pub fn set_weight(&mut self, name: &str, mut weight: f32) {
        if let Some(target) = self.morphs.get(name) {
            if self.clamp {
                weight = weight.clamp(target.min_value, target.max_value);
            }
            self.weights.insert(name.to_string(), weight);
        }
    }

    pub fn get_weight(&self, name: &str) -> f32 {
        self.weights.get(name).copied().unwrap_or(0.0)
    }

    pub fn reset_weights(&mut self) {
        for (name, target) in &self.morphs {
            self.weights.insert(name.clone(), target.default_value);
        }
    }

    /// Evaluates morph deltas and returns updated 3D vertex positions.
    /// Applies post-evaluation Cotangent Laplacian relaxation with volume preservation.
    pub fn evaluate(&self) -> Vec<Vec3> {
        let num_verts = self.base_mesh.vertices.len();
        let mut result = self.base_mesh.vertices.clone();

        // Collect active morphs with non-zero weight
        let active_morphs: Vec<(&MorphTarget, f32)> = self
            .morphs
            .values()
            .filter_map(|target| {
                let w = self.weights.get(&target.name).copied().unwrap_or(0.0);
                if w.abs() > 1e-6 {
                    Some((target, w))
                } else {
                    None
                }
            })
            .collect();

        if !active_morphs.is_empty() {
            // Apply sparse deltas directly
            for (target, weight) in &active_morphs {
                match &target.delta {
                    MorphDelta::Sparse(deltas) => {
                        for delta in deltas {
                            if delta.vertex_index < num_verts {
                                result[delta.vertex_index] += delta.offset * (*weight);
                            }
                        }
                    }
                    MorphDelta::Dense(_) => {}
                }
            }

            // Parallel dense morph combination over vertex chunks
            let dense_active: Vec<(&[Vec3], f32)> = active_morphs
                .iter()
                .filter_map(|(target, w)| match &target.delta {
                    MorphDelta::Dense(deltas) if deltas.len() == num_verts => {
                        Some((deltas.as_slice(), *w))
                    }
                    _ => None,
                })
                .collect();

            if !dense_active.is_empty() {
                result.par_iter_mut().enumerate().for_each(|(i, v)| {
                    let mut acc = Vec3::ZERO;
                    for (deltas, weight) in &dense_active {
                        acc += deltas[i] * (*weight);
                    }
                    *v += acc;
                });
            }
        }

        // Apply post-evaluation Cotangent Laplacian relaxation pass
        if self.enable_relaxation && !self.base_mesh.polygons.is_empty() {
            self.relax(&mut result);
        }

        result
    }

    /// Performs Cotangent Laplacian relaxation with iterative volume-preservation constraint.
    pub fn relax(&self, vertices: &mut Vec<Vec3>) {
        if !self.enable_relaxation
            || self.max_iterations == 0
            || self.base_mesh.polygons.is_empty()
            || vertices.is_empty()
        {
            return;
        }

        let cache = match &self.laplacian_cache {
            Some(c) => c,
            None => return,
        };

        let target_vol = calculate_enclosed_volume(vertices, &self.base_mesh.polygons);
        if target_vol <= 1e-8 {
            return;
        }

        let num_verts = vertices.len();

        cache.with_scratch_buffer(|laplacian_step| {
            for _pass in 0..self.max_iterations {
                cache.compute_laplacian_vectors(vertices, laplacian_step);

                let mut max_move_sq: f32 = 0.0;
                for i in 0..num_verts {
                    let shift = unsafe { *laplacian_step.get_unchecked(i) } * self.relaxation_factor;
                    let move_sq = shift.length_squared();
                    if move_sq > max_move_sq {
                        max_move_sq = move_sq;
                    }
                    unsafe {
                        *vertices.get_unchecked_mut(i) += shift;
                    }
                }

                // Volume preservation penalty constraint
                let (current_vol, vol_center) =
                    calculate_volume_and_center(vertices, &self.base_mesh.polygons);

                if current_vol > 1e-8 {
                    let scale = (target_vol / current_vol).cbrt();
                    vertices.par_iter_mut().for_each(|v| {
                        *v = vol_center + (*v - vol_center) * scale;
                    });
                }

                let vol_divergence = ((current_vol - target_vol) / target_vol).abs();
                if max_move_sq.sqrt() < self.convergence_threshold
                    && vol_divergence < self.convergence_threshold
                {
                    break;
                }
            }
        });
    }

    /// Computes enclosed 3D mesh volume for a vertex buffer.
    pub fn calculate_volume(&self, vertices: &[Vec3]) -> f32 {
        calculate_enclosed_volume(vertices, &self.base_mesh.polygons)
    }

    /// Computes volumetric center for a vertex buffer.
    pub fn calculate_volumetric_center(&self, vertices: &[Vec3]) -> Vec3 {
        calculate_volumetric_center(vertices, &self.base_mesh.polygons)
    }

    /// Computes vertex surface normals for a vertex buffer.
    pub fn calculate_surface_normals(&self, vertices: &[Vec3]) -> Vec<Vec3> {
        calculate_surface_normals(vertices, &self.base_mesh.polygons)
    }

    /// Linear interpolation / continuous mix factor between preset weight vectors
    pub fn mix_presets(&mut self, preset_a: &HashMap<String, f32>, preset_b: &HashMap<String, f32>, mix_factor: f32) {
        let factor = mix_factor.clamp(0.0, 1.0);
        let all_keys: std::collections::HashSet<&String> = preset_a.keys().chain(preset_b.keys()).collect();
        for key in all_keys {
            let val_a = preset_a.get(key).copied().unwrap_or(0.0);
            let val_b = preset_b.get(key).copied().unwrap_or(0.0);
            let blended = val_a * (1.0 - factor) + val_b * factor;
            self.set_weight(key, blended);
        }
    }
}
