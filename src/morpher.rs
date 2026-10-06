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
}

impl Morpher {
    pub fn new(base_mesh: BaseMesh) -> Self {
        Self {
            base_mesh,
            morphs: HashMap::new(),
            weights: HashMap::new(),
            clamp: true,
        }
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
    /// Uses parallel iteration over vertices for sub-millisecond evaluation on 50k+ vertices.
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

        if active_morphs.is_empty() {
            return result;
        }

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
                MorphDelta::Dense(deltas) if deltas.len() == num_verts => Some((deltas.as_slice(), *w)),
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

        result
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
