use crate::spatial::{BVHTree, KDTree};
use glam::Vec3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FittingConfig {
    pub max_distance: f32,
    pub smoothing_iterations: usize,
    pub ray_cast_direction: Vec3,
    pub use_raycast: bool,
}

impl Default for FittingConfig {
    fn default() -> Self {
        Self {
            max_distance: 0.1,
            smoothing_iterations: 2,
            ray_cast_direction: Vec3::NEG_Y,
            use_raycast: true,
        }
    }
}

pub struct Fitter {
    bvh: BVHTree,
    kdtree: KDTree,
    _body_vertices: Vec<Vec3>,
}

impl Fitter {
    pub fn new(body_vertices: Vec<Vec3>, body_polygons: &[[usize; 3]]) -> Self {
        let bvh = BVHTree::FromPolygons(&body_vertices, body_polygons);
        let kdtree = KDTree::build(&body_vertices);
        Self {
            bvh,
            kdtree,
            _body_vertices: body_vertices,
        }
    }

    /// Fits asset vertices to the body mesh surface using spatial queries
    pub fn fit_asset(&self, asset_vertices: &[Vec3], config: &FittingConfig) -> Vec<Vec3> {
        let mut fitted: Vec<Vec3> = asset_vertices
            .par_iter()
            .map(|&p| {
                if config.use_raycast {
                    // Try raycast along direction first
                    if let Some(hit) = self.bvh.ray_cast(p, config.ray_cast_direction, config.max_distance) {
                        return hit.point;
                    }
                    // Try reverse raycast
                    if let Some(hit) = self.bvh.ray_cast(p, -config.ray_cast_direction, config.max_distance) {
                        return hit.point;
                    }
                }

                // Fall back to nearest BVH surface or KDTree nearest point
                if let Some(hit) = self.bvh.find_nearest(p, config.max_distance) {
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

        // Apply laplacian smoothing passes on fitted surface points
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
                            *p = *p * 0.5 + avg * 0.5;
                        }
                    }
                }
            });
            fitted = smoothed;
        }

        fitted
    }
}
