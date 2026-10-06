use glam::{Mat4, Quat, Vec3};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bone {
    pub name: String,
    pub parent: Option<String>,
    pub head: Vec3,
    pub tail: Vec3,
    pub matrix_local: [f32; 16],
}

impl Bone {
    pub fn new(name: impl Into<String>, head: Vec3, tail: Vec3, parent: Option<String>) -> Self {
        let name = name.into();
        let dir = (tail - head).normalize_or_zero();
        let up = if dir.y.abs() < 0.999 { Vec3::Y } else { Vec3::Z };
        let mat = Mat4::look_at_lh(head, tail, up).inverse();
        Self {
            name,
            parent,
            head,
            tail,
            matrix_local: mat.to_cols_array(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonePose {
    pub translation: Vec3,
    pub rotation: [f32; 4], // Quaternion (x, y, z, w)
    pub scale: Vec3,
}

impl Default for BonePose {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY.to_array(),
            scale: Vec3::ONE,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VertexWeight {
    pub bone_index: usize,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
    pub bone_indices: HashMap<String, usize>,
}

impl Skeleton {
    pub fn new(bones: Vec<Bone>) -> Self {
        let mut bone_indices = HashMap::new();
        for (idx, b) in bones.iter().enumerate() {
            bone_indices.insert(b.name.clone(), idx);
        }
        Self { bones, bone_indices }
    }

    /// Calculates global pose matrices for each bone given pose overrides
    pub fn compute_pose_matrices(&self, poses: &HashMap<String, BonePose>) -> Vec<Mat4> {
        let num_bones = self.bones.len();
        let mut global_matrices = vec![Mat4::IDENTITY; num_bones];

        for i in 0..num_bones {
            let bone = &self.bones[i];
            let pose = poses.get(&bone.name).cloned().unwrap_or_default();
            let q = Quat::from_array(pose.rotation);
            let local_pose_mat = Mat4::from_scale_rotation_translation(pose.scale, q, pose.translation);
            let rest_mat = Mat4::from_cols_array(&bone.matrix_local);

            let bone_local = rest_mat * local_pose_mat;

            if let Some(ref parent_name) = bone.parent {
                if let Some(&parent_idx) = self.bone_indices.get(parent_name) {
                    global_matrices[i] = global_matrices[parent_idx] * bone_local;
                } else {
                    global_matrices[i] = bone_local;
                }
            } else {
                global_matrices[i] = bone_local;
            }
        }

        global_matrices
    }

    /// Linear Blend Skinning (LBS)
    pub fn skin_vertices(
        &self,
        vertices: &[Vec3],
        weights: &[Vec<VertexWeight>],
        pose_matrices: &[Mat4],
    ) -> Vec<Vec3> {
        if vertices.len() != weights.len() {
            return vertices.to_vec();
        }

        vertices
            .par_iter()
            .zip(weights.par_iter())
            .map(|(&v, v_weights)| {
                if v_weights.is_empty() {
                    return v;
                }
                let mut skinned_pos = Vec3::ZERO;
                let mut total_weight = 0.0;

                for w in v_weights {
                    if w.bone_index < pose_matrices.len() {
                        let m = pose_matrices[w.bone_index];
                        skinned_pos += m.transform_point3(v) * w.weight;
                        total_weight += w.weight;
                    }
                }

                if total_weight > 1e-6 {
                    skinned_pos / total_weight
                } else {
                    v
                }
            })
            .collect()
    }
}
