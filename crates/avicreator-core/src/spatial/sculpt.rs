use crate::mesh::Mesh;
use crate::spatial::bvh::BVHTree;
use crate::vector_math::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SculptMode {
    Push,
    Pull,
    Smooth,
}

impl SculptMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pull" => SculptMode::Pull,
            "smooth" => SculptMode::Smooth,
            _ => SculptMode::Push,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalloffCurve {
    Gaussian,
    Smoothstep,
}

impl FalloffCurve {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "smoothstep" => FalloffCurve::Smoothstep,
            _ => FalloffCurve::Gaussian,
        }
    }

    pub fn evaluate(&self, distance: f32, radius: f32) -> f32 {
        if radius <= 0.0 {
            return 0.0;
        }
        let t = (distance / radius).clamp(0.0, 1.0);
        match self {
            FalloffCurve::Smoothstep => 1.0 - (3.0 * t * t - 2.0 * t * t * t),
            FalloffCurve::Gaussian => {
                let e_neg3 = (-3.0f32).exp();
                let val = (-3.0 * t * t).exp();
                ((val - e_neg3) / (1.0 - e_neg3)).clamp(0.0, 1.0)
            }
        }
    }
}

pub struct SculptContext {
    visited_tris: Vec<usize>,
    visited_verts: Vec<usize>,
    vert_flags: Vec<bool>,
}

impl SculptContext {
    pub fn new() -> Self {
        Self {
            visited_tris: Vec::with_capacity(1024),
            visited_verts: Vec::with_capacity(1024),
            vert_flags: Vec::new(),
        }
    }

    pub fn apply_stroke(
        &mut self,
        mesh: &mut Mesh,
        bvh: &BVHTree,
        center: Vec3,
        radius: f32,
        strength: f32,
        mode: SculptMode,
        falloff: FalloffCurve,
        direction: Vec3,
    ) {
        if mesh.positions.is_empty() || radius <= 0.0 || strength <= 0.0 {
            return;
        }

        if self.vert_flags.len() < mesh.positions.len() {
            self.vert_flags.resize(mesh.positions.len(), false);
        }

        bvh.query_radius(
            center,
            radius,
            &mesh.positions,
            &mut self.visited_tris,
            &mut self.visited_verts,
            &mut self.vert_flags,
        );

        if self.visited_verts.is_empty() {
            return;
        }

        let dir = if direction.length_squared() > 1e-6 {
            direction.normalize()
        } else {
            Vec3::Y
        };

        match mode {
            SculptMode::Push => {
                for &vi in &self.visited_verts {
                    let pos = mesh.positions[vi];
                    let dist = pos.sub(center).length();
                    let w = falloff.evaluate(dist, radius);
                    let norm = if direction.length_squared() > 1e-6 { dir } else { mesh.normals[vi] };
                    let disp = norm.scale(strength * w);
                    mesh.positions[vi] = pos.add(disp);
                }
            }
            SculptMode::Pull => {
                for &vi in &self.visited_verts {
                    let pos = mesh.positions[vi];
                    let dist = pos.sub(center).length();
                    let w = falloff.evaluate(dist, radius);
                    let norm = if direction.length_squared() > 1e-6 { dir } else { mesh.normals[vi] };
                    let disp = norm.scale(-strength * w);
                    mesh.positions[vi] = pos.add(disp);
                }
            }
            SculptMode::Smooth => {
                let mut avg_pos = Vec3::ZERO;
                let mut count = 0.0f32;
                for &vi in &self.visited_verts {
                    avg_pos = avg_pos.add(mesh.positions[vi]);
                    count += 1.0;
                }
                if count > 0.0 {
                    let center_avg = avg_pos.scale(1.0 / count);
                    for &vi in &self.visited_verts {
                        let pos = mesh.positions[vi];
                        let dist = pos.sub(center).length();
                        let w = falloff.evaluate(dist, radius);
                        let target = pos.lerp(center_avg, (strength * w).clamp(0.0, 1.0));
                        mesh.positions[vi] = target;
                    }
                }
            }
        }

        // Incremental surface normal recalculation for changed mesh faces
        let mut face_normals = vec![Vec3::ZERO; self.visited_tris.len()];
        for (idx, &tri_idx) in self.visited_tris.iter().enumerate() {
            let chunk_idx = tri_idx * 3;
            if chunk_idx + 2 < mesh.indices.len() {
                let i0 = mesh.indices[chunk_idx] as usize;
                let i1 = mesh.indices[chunk_idx + 1] as usize;
                let i2 = mesh.indices[chunk_idx + 2] as usize;

                if i0 < mesh.positions.len() && i1 < mesh.positions.len() && i2 < mesh.positions.len() {
                    let v0 = mesh.positions[i0];
                    let v1 = mesh.positions[i1];
                    let v2 = mesh.positions[i2];
                    let e1 = v1.sub(v0);
                    let e2 = v2.sub(v0);
                    face_normals[idx] = e1.cross(e2);
                }
            }
        }

        for (idx, &tri_idx) in self.visited_tris.iter().enumerate() {
            let chunk_idx = tri_idx * 3;
            if chunk_idx + 2 < mesh.indices.len() {
                let fnorm = face_normals[idx];
                for k in 0..3 {
                    let vi = mesh.indices[chunk_idx + k] as usize;
                    if vi < mesh.normals.len() {
                        mesh.normals[vi] = mesh.normals[vi].add(fnorm).normalize();
                    }
                }
            }
        }
    }
}
