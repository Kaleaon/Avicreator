use crate::spatial::{BVHTree, KDTree};
use glam::Vec3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FittingConfig {
    pub max_distance: f32,
    pub min_clearance: f32,
    pub smoothing_iterations: usize,
    pub ray_cast_direction: Vec3,
    pub use_raycast: bool,
}

impl Default for FittingConfig {
    fn default() -> Self {
        Self {
            max_distance: 0.1,
            min_clearance: 0.002,
            smoothing_iterations: 2,
            ray_cast_direction: Vec3::NEG_Y,
            use_raycast: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GarmentLayer {
    pub layer_depth: u32,
    pub vertices: Vec<Vec3>,
    pub polygons: Vec<[usize; 3]>,
}

pub struct Fitter {
    bvh: BVHTree,
    kdtree: KDTree,
    body_vertices: Vec<Vec3>,
    body_polygons: Vec<[usize; 3]>,
    inner_layers: Vec<GarmentLayer>,
}

impl Fitter {
    pub fn new(body_vertices: Vec<Vec3>, body_polygons: &[[usize; 3]]) -> Self {
        let bvh = BVHTree::FromPolygons(&body_vertices, body_polygons);
        let kdtree = KDTree::build(&body_vertices);
        Self {
            bvh,
            kdtree,
            body_vertices,
            body_polygons: body_polygons.to_vec(),
            inner_layers: Vec::new(),
        }
    }

    pub fn add_inner_layer(&mut self, layer: GarmentLayer) {
        self.inner_layers.push(layer);
    }

    pub fn clear_inner_layers(&mut self) {
        self.inner_layers.clear();
    }

    pub fn build_composite_bvh(&self, target_layer_depth: u32) -> BVHTree {
        let active_layers: Vec<&GarmentLayer> = self
            .inner_layers
            .iter()
            .filter(|l| l.layer_depth < target_layer_depth)
            .collect();

        if active_layers.is_empty() {
            return self.bvh.clone();
        }

        let mut composite_verts = self.body_vertices.clone();
        let mut composite_polys = self.body_polygons.clone();
        let mut current_offset = composite_verts.len();

        for layer in active_layers {
            composite_verts.extend_from_slice(&layer.vertices);
            for poly in &layer.polygons {
                composite_polys.push([
                    poly[0] + current_offset,
                    poly[1] + current_offset,
                    poly[2] + current_offset,
                ]);
            }
            current_offset += layer.vertices.len();
        }

        BVHTree::FromPolygons(&composite_verts, &composite_polys)
    }

    /// Fits asset vertices to the body mesh surface using spatial queries
    pub fn fit_asset(&self, asset_vertices: &[Vec3], config: &FittingConfig) -> Vec<Vec3> {
        self.fit_asset_layer(asset_vertices, config, u32::MAX)
    }

    /// Fits asset vertices against the composite multi-layer collision stack
    pub fn fit_asset_layer(
        &self,
        asset_vertices: &[Vec3],
        config: &FittingConfig,
        layer_depth: u32,
    ) -> Vec<Vec3> {
        let composite_bvh = self.build_composite_bvh(layer_depth);

        let active_layers: Vec<&GarmentLayer> = self
            .inner_layers
            .iter()
            .filter(|l| l.layer_depth < layer_depth)
            .collect();

        let mut active_bvhs: Vec<BVHTree> = Vec::with_capacity(1 + active_layers.len());
        active_bvhs.push(self.bvh.clone());
        for layer in active_layers {
            active_bvhs.push(BVHTree::FromPolygons(&layer.vertices, &layer.polygons));
        }

        let mut fitted: Vec<Vec3> = asset_vertices
            .par_iter()
            .map(|&p| {
                if config.use_raycast {
                    // Try raycast along direction first
                    if let Some(hit) = composite_bvh.ray_cast(p, config.ray_cast_direction, config.max_distance) {
                        return hit.point;
                    }
                    // Try reverse raycast
                    if let Some(hit) = composite_bvh.ray_cast(p, -config.ray_cast_direction, config.max_distance) {
                        return hit.point;
                    }
                }

                // Fall back to nearest BVH surface or KDTree nearest point
                if let Some(hit) = composite_bvh.find_nearest(p, config.max_distance) {
                    hit.point
                } else if let Some((kd_pt, _, dist)) = self.kdtree.find_nearest(p) {
                    if dist <= config.max_distance * 2.0 {
                        kd_pt
                    } else {
                        p
                    }
                } else {
                    p
                }
            })
            .collect();

        // Surface clearance projection against all active collision stack surface layers
        if config.min_clearance > 0.0 {
            fitted.par_iter_mut().for_each(|p| {
                for bvh in &active_bvhs {
                    if let Some(hit) = bvh.find_nearest(*p, config.max_distance * 2.0) {
                        let norm = hit.normal;
                        let norm_len = norm.length();
                        if norm_len > 1e-6 {
                            let unit_norm = norm / norm_len;
                            let v_vec = *p - hit.point;
                            let signed_dist = v_vec.dot(unit_norm);
                            if signed_dist < config.min_clearance {
                                *p = hit.point + unit_norm * config.min_clearance;
                            }
                        }
                    }
                }
            });
        }

        // Apply laplacian smoothing passes on fitted surface points relative to active layers
        for _ in 0..config.smoothing_iterations {
            let mut smoothed = fitted.clone();
            smoothed.par_iter_mut().enumerate().for_each(|(_i, p)| {
                if let Some((_, _, dist)) = self.kdtree.find_nearest(*p) {
                    if dist < config.max_distance {
                        let neighbors = self.kdtree.find_range(*p, config.max_distance * 0.5);
                        if !neighbors.is_empty() {
                            let mut sum = Vec3::ZERO;
                            for (n_pt, _, _) in &neighbors {
                                sum += *n_pt;
                            }
                            let avg = sum / (neighbors.len() as f32);
                            let mut candidate = *p * 0.5 + avg * 0.5;

                            if config.min_clearance > 0.0 {
                                for bvh in &active_bvhs {
                                    if let Some(hit) = bvh.find_nearest(candidate, config.max_distance * 2.0) {
                                        let norm = hit.normal;
                                        let norm_len = norm.length();
                                        if norm_len > 1e-6 {
                                            let unit_norm = norm / norm_len;
                                            let v_vec = candidate - hit.point;
                                            let signed_dist = v_vec.dot(unit_norm);
                                            if signed_dist < config.min_clearance {
                                                candidate = hit.point + unit_norm * config.min_clearance;
                                            }
                                        }
                                    }
                                }
                            }
                            *p = candidate;
                        }
                    }
                }
            });
            fitted = smoothed;
        }

        fitted
    }
}
