use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        let dir = if direction.length_squared() > 1e-12 {
            direction.normalize()
        } else {
            Vec3::Z
        };
        Self {
            origin,
            direction: dir,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
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
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn grow_aabb(&mut self, other: &AABB) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn intersects_ray(&self, ray: &Ray, max_dist: f32) -> bool {
        let inv_dir = Vec3::new(
            1.0 / if ray.direction.x.abs() < 1e-12 { 1e-12 } else { ray.direction.x },
            1.0 / if ray.direction.y.abs() < 1e-12 { 1e-12 } else { ray.direction.y },
            1.0 / if ray.direction.z.abs() < 1e-12 { 1e-12 } else { ray.direction.z },
        );

        let t1 = (self.min.x - ray.origin.x) * inv_dir.x;
        let t2 = (self.max.x - ray.origin.x) * inv_dir.x;
        let t3 = (self.min.y - ray.origin.y) * inv_dir.y;
        let t4 = (self.max.y - ray.origin.y) * inv_dir.y;
        let t5 = (self.min.z - ray.origin.z) * inv_dir.z;
        let t6 = (self.max.z - ray.origin.z) * inv_dir.z;

        let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
        let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

        if tmax < 0.0 || tmin > tmax {
            return false;
        }

        tmin <= max_dist
    }

    pub fn sq_dist_to_point(&self, p: Vec3) -> f32 {
        let mut sq_dist = 0.0;
        for i in 0..3 {
            let v = p[i];
            if v < self.min[i] {
                let d = self.min[i] - v;
                sq_dist += d * d;
            } else if v > self.max[i] {
                let d = v - self.max[i];
                sq_dist += d * d;
            }
        }
        sq_dist
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Triangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub index: usize,
}

impl Triangle {
    pub fn new(v0: Vec3, v1: Vec3, v2: Vec3, index: usize) -> Self {
        Self { v0, v1, v2, index }
    }

    pub fn normal(&self) -> Vec3 {
        let e1 = self.v1 - self.v0;
        let e2 = self.v2 - self.v0;
        let n = e1.cross(e2);
        if n.length_squared() > 1e-12 {
            n.normalize()
        } else {
            Vec3::Z
        }
    }

    pub fn aabb(&self) -> AABB {
        AABB::from_points(&[self.v0, self.v1, self.v2])
    }

    pub fn center(&self) -> Vec3 {
        (self.v0 + self.v1 + self.v2) / 3.0
    }

    /// Möller–Trumbore ray-triangle intersection
    pub fn intersect_ray(&self, ray: &Ray, max_dist: f32) -> Option<RayHit> {
        let edge1 = self.v1 - self.v0;
        let edge2 = self.v2 - self.v0;
        let h = ray.direction.cross(edge2);
        let a = edge1.dot(h);

        if a.abs() < 1e-8 {
            return None; // Ray is parallel to triangle
        }

        let f = 1.0 / a;
        let s = ray.origin - self.v0;
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
            let hit_point = ray.origin + ray.direction * t;
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

    /// Finds nearest point on triangle to a given point p
    pub fn nearest_point(&self, p: Vec3) -> (Vec3, Vec3, f32) {
        let ab = self.v1 - self.v0;
        let ac = self.v2 - self.v0;
        let ap = p - self.v0;

        let d1 = ab.dot(ap);
        let d2 = ac.dot(ap);
        if d1 <= 0.0 && d2 <= 0.0 {
            let pt = self.v0;
            return (pt, self.normal(), p.distance(pt));
        }

        let bp = p - self.v1;
        let d3 = ab.dot(bp);
        let d4 = ac.dot(bp);
        if d3 >= 0.0 && d4 <= d3 {
            let pt = self.v1;
            return (pt, self.normal(), p.distance(pt));
        }

        let vc = d1 * d4 - d3 * d2;
        if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
            let v = d1 / (d1 - d3);
            let pt = self.v0 + ab * v;
            return (pt, self.normal(), p.distance(pt));
        }

        let cp = p - self.v2;
        let d5 = ab.dot(cp);
        let d6 = ac.dot(cp);
        if d6 >= 0.0 && d5 <= d6 {
            let pt = self.v2;
            return (pt, self.normal(), p.distance(pt));
        }

        let vb = d5 * d2 - d1 * d6;
        if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
            let w = d2 / (d2 - d6);
            let pt = self.v0 + ac * w;
            return (pt, self.normal(), p.distance(pt));
        }

        let va = d3 * d6 - d5 * d4;
        if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
            let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
            let pt = self.v1 + (self.v2 - self.v1) * w;
            return (pt, self.normal(), p.distance(pt));
        }

        let denom = 1.0 / (va + vb + vc);
        let v = vb * denom;
        let w = vc * denom;
        let pt = self.v0 + ab * v + ac * w;
        (pt, self.normal(), p.distance(pt))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RayHit {
    pub point: Vec3,
    pub normal: Vec3,
    pub face_index: usize,
    pub distance: f32,
}

pub fn transform_point(m: &Mat4, p: Vec3) -> Vec3 {
    m.transform_point3(p)
}

pub fn transform_vector(m: &Mat4, v: Vec3) -> Vec3 {
    m.transform_vector3(v)
}
