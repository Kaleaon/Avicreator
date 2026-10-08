use crate::vector_math::Vec3;
use crate::mesh::Mesh;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        let dir = direction.normalize();
        let dir = if dir.length_squared() < 1e-12 { Vec3::Z } else { dir };
        Self { origin, direction: dir }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub fn empty() -> Self {
        Self {
            min: Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
            max: Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
        }
    }

    pub fn from_points(points: &[Vec3]) -> Self {
        let mut aabb = Self::empty();
        for &p in points {
            aabb.grow_point(p);
        }
        aabb
    }

    pub fn grow_point(&mut self, p: Vec3) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.min.z = self.min.z.min(p.z);

        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
        self.max.z = self.max.z.max(p.z);
    }

    pub fn grow_aabb(&mut self, other: &AABB) {
        self.min.x = self.min.x.min(other.min.x);
        self.min.y = self.min.y.min(other.min.y);
        self.min.z = self.min.z.min(other.min.z);

        self.max.x = self.max.x.max(other.max.x);
        self.max.y = self.max.y.max(other.max.y);
        self.max.z = self.max.z.max(other.max.z);
    }

    pub fn center(&self) -> Vec3 {
        self.min.add(self.max).scale(0.5)
    }

    pub fn intersects_ray(&self, ray: &Ray, max_dist: f32) -> bool {
        let inv_dx = if ray.direction.x.abs() < 1e-12 { 1e12 } else { 1.0 / ray.direction.x };
        let inv_dy = if ray.direction.y.abs() < 1e-12 { 1e12 } else { 1.0 / ray.direction.y };
        let inv_dz = if ray.direction.z.abs() < 1e-12 { 1e12 } else { 1.0 / ray.direction.z };

        let t1 = (self.min.x - ray.origin.x) * inv_dx;
        let t2 = (self.max.x - ray.origin.x) * inv_dx;
        let t3 = (self.min.y - ray.origin.y) * inv_dy;
        let t4 = (self.max.y - ray.origin.y) * inv_dy;
        let t5 = (self.min.z - ray.origin.z) * inv_dz;
        let t6 = (self.max.z - ray.origin.z) * inv_dz;

        let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
        let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

        if tmax < 0.0 || tmin > tmax {
            return false;
        }

        tmin <= max_dist
    }

    pub fn sq_dist_to_point(&self, p: Vec3) -> f32 {
        let mut sq_dist = 0.0;
        
        if p.x < self.min.x {
            let d = self.min.x - p.x;
            sq_dist += d * d;
        } else if p.x > self.max.x {
            let d = p.x - self.max.x;
            sq_dist += d * d;
        }

        if p.y < self.min.y {
            let d = self.min.y - p.y;
            sq_dist += d * d;
        } else if p.y > self.max.y {
            let d = p.y - self.max.y;
            sq_dist += d * d;
        }

        if p.z < self.min.z {
            let d = self.min.z - p.z;
            sq_dist += d * d;
        } else if p.z > self.max.z {
            let d = p.z - self.max.z;
            sq_dist += d * d;
        }

        sq_dist
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub i0: usize,
    pub i1: usize,
    pub i2: usize,
    pub index: usize,
}

impl Triangle {
    pub fn new(v0: Vec3, v1: Vec3, v2: Vec3, i0: usize, i1: usize, i2: usize, index: usize) -> Self {
        Self { v0, v1, v2, i0, i1, i2, index }
    }

    pub fn normal(&self) -> Vec3 {
        let e1 = self.v1.sub(self.v0);
        let e2 = self.v2.sub(self.v0);
        let n = e1.cross(e2);
        if n.length_squared() > 1e-12 {
            n.normalize()
        } else {
            Vec3::Y
        }
    }

    pub fn aabb(&self) -> AABB {
        AABB::from_points(&[self.v0, self.v1, self.v2])
    }

    pub fn center(&self) -> Vec3 {
        self.v0.add(self.v1).add(self.v2).scale(1.0 / 3.0)
    }

    /// Möller–Trumbore ray-triangle intersection
    pub fn intersect_ray(&self, ray: &Ray, max_dist: f32) -> Option<RayHit> {
        let edge1 = self.v1.sub(self.v0);
        let edge2 = self.v2.sub(self.v0);
        let h = ray.direction.cross(edge2);
        let a = edge1.dot(h);

        if a.abs() < 1e-8 {
            return None;
        }

        let f = 1.0 / a;
        let s = ray.origin.sub(self.v0);
        let u = f * s.dot(h);
        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = s.cross(edge1);
        let v = f * ray.direction.dot(q);
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * edge2.dot(q);
        if t > 1e-7 && t <= max_dist {
            let hit_point = ray.origin.add(ray.direction.scale(t));
            let normal = self.normal();
            Some(RayHit {
                point: hit_point,
                normal,
                face_index: self.index,
                distance: t,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub point: Vec3,
    pub normal: Vec3,
    pub face_index: usize,
    pub distance: f32,
}

#[derive(Debug, Clone)]
enum BVHNode {
    Leaf {
        aabb: AABB,
        triangles: Vec<Triangle>,
    },
    Internal {
        aabb: AABB,
        left: Box<BVHNode>,
        right: Box<BVHNode>,
    },
}

impl BVHNode {
    fn aabb(&self) -> AABB {
        match self {
            BVHNode::Leaf { aabb, .. } => *aabb,
            BVHNode::Internal { aabb, .. } => *aabb,
        }
    }

    fn build(mut triangles: Vec<Triangle>, depth: usize) -> Self {
        let mut aabb = AABB::empty();
        for tri in &triangles {
            aabb.grow_aabb(&tri.aabb());
        }

        if triangles.len() <= 4 || depth > 24 {
            return BVHNode::Leaf { aabb, triangles };
        }

        let size = aabb.max.sub(aabb.min);
        let mut axis = 0;
        if size.y > size.x {
            axis = 1;
        }
        let max_so_far = if axis == 0 { size.x } else { size.y };
        if size.z > max_so_far {
            axis = 2;
        }

        triangles.sort_by(|a, b| {
            let ca = match axis {
                0 => a.center().x,
                1 => a.center().y,
                _ => a.center().z,
            };
            let cb = match axis {
                0 => b.center().x,
                1 => b.center().y,
                _ => b.center().z,
            };
            ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mid = triangles.len() / 2;
        let right_tris = triangles.split_off(mid);

        let left = Box::new(BVHNode::build(triangles, depth + 1));
        let right = Box::new(BVHNode::build(right_tris, depth + 1));

        BVHNode::Internal { aabb, left, right }
    }

    fn ray_cast(&self, ray: &Ray, max_dist: f32) -> Option<RayHit> {
        if !self.aabb().intersects_ray(ray, max_dist) {
            return None;
        }

        match self {
            BVHNode::Leaf { triangles, .. } => {
                let mut best_hit: Option<RayHit> = None;
                let mut current_max = max_dist;
                for tri in triangles {
                    if let Some(hit) = tri.intersect_ray(ray, current_max) {
                        current_max = hit.distance;
                        best_hit = Some(hit);
                    }
                }
                best_hit
            }
            BVHNode::Internal { left, right, .. } => {
                let hit_left = left.ray_cast(ray, max_dist);
                let hit_right = right.ray_cast(
                    ray,
                    hit_left.as_ref().map_or(max_dist, |h| h.distance),
                );

                match (hit_left, hit_right) {
                    (Some(l), Some(r)) => {
                        if l.distance <= r.distance {
                            Some(l)
                        } else {
                            Some(r)
                        }
                    }
                    (Some(l), None) => Some(l),
                    (None, Some(r)) => Some(r),
                    (None, None) => None,
                }
            }
        }
    }

    fn query_radius_vertices(
        &self,
        center: Vec3,
        radius_sq: f32,
        positions: &[Vec3],
        visited_tris: &mut Vec<usize>,
        visited_verts: &mut Vec<usize>,
        vert_flags: &mut [bool],
    ) {
        if self.aabb().sq_dist_to_point(center) > radius_sq {
            return;
        }

        match self {
            BVHNode::Leaf { triangles, .. } => {
                for tri in triangles {
                    visited_tris.push(tri.index);
                    for &vi in &[tri.i0, tri.i1, tri.i2] {
                        if vi < positions.len() && !vert_flags[vi] {
                            let pos = positions[vi];
                            let diff = pos.sub(center);
                            if diff.length_squared() <= radius_sq {
                                vert_flags[vi] = true;
                                visited_verts.push(vi);
                            }
                        }
                    }
                }
            }
            BVHNode::Internal { left, right, .. } => {
                left.query_radius_vertices(center, radius_sq, positions, visited_tris, visited_verts, vert_flags);
                right.query_radius_vertices(center, radius_sq, positions, visited_tris, visited_verts, vert_flags);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct BVHTree {
    root: Option<BVHNode>,
    pub num_triangles: usize,
}

impl BVHTree {
    pub fn empty() -> Self {
        Self {
            root: None,
            num_triangles: 0,
        }
    }

    pub fn from_mesh(mesh: &Mesh) -> Self {
        let mut triangles = Vec::with_capacity(mesh.indices.len() / 3);
        for (tri_idx, chunk) in mesh.indices.chunks_exact(3).enumerate() {
            let i0 = chunk[0] as usize;
            let i1 = chunk[1] as usize;
            let i2 = chunk[2] as usize;
            if i0 < mesh.positions.len() && i1 < mesh.positions.len() && i2 < mesh.positions.len() {
                triangles.push(Triangle::new(
                    mesh.positions[i0],
                    mesh.positions[i1],
                    mesh.positions[i2],
                    i0, i1, i2,
                    tri_idx,
                ));
            }
        }

        let num_triangles = triangles.len();
        if triangles.is_empty() {
            return Self::empty();
        }

        let root = BVHNode::build(triangles, 0);
        Self {
            root: Some(root),
            num_triangles,
        }
    }

    pub fn ray_cast(&self, origin: Vec3, direction: Vec3, max_dist: f32) -> Option<RayHit> {
        let ray = Ray::new(origin, direction);
        let dist = if max_dist <= 0.0 { f32::INFINITY } else { max_dist };
        self.root.as_ref().and_then(|r| r.ray_cast(&ray, dist))
    }

    pub fn query_radius(
        &self,
        center: Vec3,
        radius: f32,
        positions: &[Vec3],
        visited_tris: &mut Vec<usize>,
        visited_verts: &mut Vec<usize>,
        vert_flags: &mut [bool],
    ) {
        visited_tris.clear();
        visited_verts.clear();
        let radius_sq = radius * radius;
        if let Some(ref root) = self.root {
            root.query_radius_vertices(center, radius_sq, positions, visited_tris, visited_verts, vert_flags);
        }
        for &vi in visited_verts.iter() {
            vert_flags[vi] = false;
        }
    }
}
