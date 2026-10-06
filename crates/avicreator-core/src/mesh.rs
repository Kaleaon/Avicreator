use crate::vector_math::{Mat4, Vec3};

#[derive(Debug, Clone, PartialEq)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: [f32; 2],
    pub bone_weights: [f32; 4],
    pub bone_indices: [u32; 4],
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            normal: Vec3::Y,
            uv: [0.0, 0.0],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
            bone_indices: [0, 0, 0, 0],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub bone_weights: Vec<[f32; 4]>,
    pub bone_indices: Vec<[u32; 4]>,
}

impl Mesh {
    pub fn new(positions: Vec<Vec3>, indices: Vec<u32>) -> Self {
        let count = positions.len();
        Self {
            positions,
            normals: vec![Vec3::Y; count],
            uvs: vec![[0.0, 0.0]; count],
            indices,
            bone_weights: vec![[1.0, 0.0, 0.0, 0.0]; count],
            bone_indices: vec![[0, 0, 0, 0]; count],
        }
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    pub fn recalculate_normals(&mut self) {
        if self.positions.is_empty() {
            return;
        }

        let mut accumulated_normals = vec![Vec3::ZERO; self.positions.len()];

        // Calculate triangle face normals and accumulate
        for chunk in self.indices.chunks_exact(3) {
            let i0 = chunk[0] as usize;
            let i1 = chunk[1] as usize;
            let i2 = chunk[2] as usize;

            if i0 < self.positions.len() && i1 < self.positions.len() && i2 < self.positions.len() {
                let v0 = self.positions[i0];
                let v1 = self.positions[i1];
                let v2 = self.positions[i2];

                let edge1 = v1.sub(v0);
                let edge2 = v2.sub(v0);
                let face_normal = edge1.cross(edge2);

                accumulated_normals[i0] = accumulated_normals[i0].add(face_normal);
                accumulated_normals[i1] = accumulated_normals[i1].add(face_normal);
                accumulated_normals[i2] = accumulated_normals[i2].add(face_normal);
            }
        }

        // Normalize
        self.normals = accumulated_normals
            .into_iter()
            .map(|n| if n.length_squared() > 1e-6 { n.normalize() } else { Vec3::Y })
            .collect();
    }

    pub fn transform(&mut self, mat: &Mat4) {
        for p in &mut self.positions {
            *p = mat.transform_point(*p);
        }
        self.recalculate_normals();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_creation_and_normals() {
        // Simple triangle in XY plane
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let indices = vec![0, 1, 2];

        let mut mesh = Mesh::new(positions, indices);
        mesh.recalculate_normals();

        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.index_count(), 3);

        // Face normal for (1,0,0) x (0,1,0) should point along +Z (0,0,1)
        assert!((mesh.normals[0].z - 1.0).abs() < 1e-5);
        assert!((mesh.normals[1].z - 1.0).abs() < 1e-5);
        assert!((mesh.normals[2].z - 1.0).abs() < 1e-5);
    }
}
