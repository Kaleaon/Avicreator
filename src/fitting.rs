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
    body_vertices: Vec<Vec3>,
}

pub fn compute_centroid(points: &[Vec3]) -> Vec3 {
    if points.is_empty() {
        return Vec3::ZERO;
    }
    let sum: Vec3 = points.iter().sum();
    sum / (points.len() as f32)
}

pub fn compute_dispersion(points: &[Vec3], centroid: Vec3) -> f32 {
    if points.is_empty() {
        return 0.0;
    }
    let sum_dist: f32 = points.iter().map(|&p| (p - centroid).length()).sum();
    sum_dist / (points.len() as f32)
}

pub fn compute_mesh_volume(points: &[Vec3], polygons: &[[usize; 3]]) -> f32 {
    if points.is_empty() || polygons.is_empty() {
        return 0.0;
    }
    let mut vol = 0.0f32;
    for poly in polygons {
        let i0 = poly[0];
        let i1 = poly[1];
        let i2 = poly[2];
        if i0 < points.len() && i1 < points.len() && i2 < points.len() {
            let p0 = points[i0];
            let p1 = points[i1];
            let p2 = points[i2];
            let cross = p1.cross(p2);
            vol += p0.dot(cross);
        }
    }
    vol / 6.0
}

impl Fitter {
    pub fn new(body_vertices: Vec<Vec3>, body_polygons: &[[usize; 3]]) -> Self {
        let bvh = BVHTree::FromPolygons(&body_vertices, body_polygons);
        let kdtree = KDTree::build(&body_vertices);
        Self {
            bvh,
            kdtree,
            body_vertices,
        }
    }

    pub fn body_vertices(&self) -> &[Vec3] {
        &self.body_vertices
    }

    /// Fits asset vertices to the body mesh surface using spatial queries
    pub fn fit_asset(&self, asset_vertices: &[Vec3], config: &FittingConfig) -> Vec<Vec3> {
        self.fit_asset_with_polygons(asset_vertices, None, config)
    }

    /// Fits asset vertices to body mesh surface with optional asset face polygons for volume preservation
    pub fn fit_asset_with_polygons(
        &self,
        asset_vertices: &[Vec3],
        asset_polygons: Option<&[[usize; 3]]>,
        config: &FittingConfig,
    ) -> Vec<Vec3> {
        if asset_vertices.is_empty() {
            return Vec::new();
        }

        let max_dist = if config.max_distance.is_finite() && config.max_distance > 0.0 {
            config.max_distance
        } else {
            0.1
        };

        let mut fitted: Vec<Vec3> = asset_vertices
            .par_iter()
            .map(|&p| {
                if config.use_raycast {
                    // Try raycast along direction first
                    if let Some(hit) = self.bvh.ray_cast(p, config.ray_cast_direction, max_dist) {
                        return hit.point;
                    }
                    // Try reverse raycast
                    if let Some(hit) = self.bvh.ray_cast(p, -config.ray_cast_direction, max_dist) {
                        return hit.point;
                    }
                }

                // Fall back to nearest BVH surface or KDTree nearest point
                if let Some(hit) = self.bvh.find_nearest(p, max_dist) {
                    hit.point
                } else if let Some((kd_pt, _, dist)) = self.kdtree.find_nearest(p) {
                    if dist <= max_dist * 2.0 {
                        kd_pt
                    } else {
                        p
                    }
                } else {
                    p
                }
            })
            .collect();

        let initial_centroid = compute_centroid(&fitted);
        let initial_dispersion = compute_dispersion(&fitted, initial_centroid);
        let initial_volume = asset_polygons
            .map(|polys| compute_mesh_volume(&fitted, polys))
            .unwrap_or(0.0);

        // Apply volume-constrained laplacian smoothing passes on fitted surface points
        let passes = config.smoothing_iterations.min(10);
        for _ in 0..passes {
            let mut smoothed = fitted.clone();
            smoothed.par_iter_mut().enumerate().for_each(|(_i, p)| {
                if let Some((_, _, dist)) = self.kdtree.find_nearest(*p) {
                    if dist < max_dist {
                        let neighbors = self.kdtree.find_range(*p, max_dist * 0.5);
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

            // Compute candidate volume & dispersion
            let cand_centroid = compute_centroid(&smoothed);
            let cand_dispersion = compute_dispersion(&smoothed, cand_centroid);
            let cand_volume = asset_polygons
                .map(|polys| compute_mesh_volume(&smoothed, polys))
                .unwrap_or(0.0);

            let mut beta = 1.0f32;
            if initial_volume.abs() > 1e-9 && cand_volume.abs() > 1e-9 {
                beta = (initial_volume.abs() / cand_volume.abs()).cbrt();
            } else if initial_dispersion > 1e-9 && cand_dispersion > 1e-9 {
                beta = initial_dispersion / cand_dispersion;
            }

            beta = beta.clamp(0.1, 10.0);

            if (beta - 1.0).abs() > 1e-6 {
                smoothed.par_iter_mut().for_each(|p| {
                    *p = cand_centroid + (*p - cand_centroid) * beta;
                });
            }

            fitted = smoothed;
        }

        fitted
    }
}
