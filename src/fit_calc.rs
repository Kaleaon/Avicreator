use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigidTransform {
    pub translation: Vec3,
    pub scale: Vec3,
    pub matrix: [f32; 16],
}

pub struct FitCalc;

impl FitCalc {
    pub fn compute_centroid(points: &[Vec3]) -> Vec3 {
        if points.is_empty() {
            return Vec3::ZERO;
        }
        let sum: Vec3 = points.iter().sum();
        sum / (points.len() as f32)
    }

    pub fn compute_bounding_box(points: &[Vec3]) -> (Vec3, Vec3) {
        if points.is_empty() {
            return (Vec3::ZERO, Vec3::ZERO);
        }
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for &p in points {
            min = min.min(p);
            max = max.max(p);
        }
        (min, max)
    }

    /// Computes rigid alignment transform from source points to target points
    pub fn align_points(source: &[Vec3], target: &[Vec3]) -> RigidTransform {
        if source.is_empty() || source.len() != target.len() {
            let m = Mat4::IDENTITY;
            return RigidTransform {
                translation: Vec3::ZERO,
                scale: Vec3::ONE,
                matrix: m.to_cols_array(),
            };
        }

        let c_src = Self::compute_centroid(source);
        let c_tgt = Self::compute_centroid(target);

        let (min_s, max_s) = Self::compute_bounding_box(source);
        let (min_t, max_t) = Self::compute_bounding_box(target);

        let size_s = (max_s - min_s).max(Vec3::splat(1e-6));
        let size_t = max_t - min_t;
        let scale = size_t / size_s;

        let translation = c_tgt - c_src * scale;
        let m = Mat4::from_scale_rotation_translation(scale, glam::Quat::IDENTITY, translation);

        RigidTransform {
            translation,
            scale,
            matrix: m.to_cols_array(),
        }
    }

    pub fn transform_points(points: &[Vec3], transform: &RigidTransform) -> Vec<Vec3> {
        let m = Mat4::from_cols_array(&transform.matrix);
        points.iter().map(|&p| m.transform_point3(p)).collect()
    }
}
