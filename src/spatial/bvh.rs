use crate::math::{Ray, RayHit, Triangle, AABB};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

        // Find split axis (longest dimension)
        let size = aabb.max - aabb.min;
        let mut axis = 0;
        if size.y > size.x {
            axis = 1;
        }
        if size.z > size[axis] {
            axis = 2;
        }

        // Sort triangles along split axis center
        triangles.sort_by(|a, b| {
            a.center()[axis]
                .partial_cmp(&b.center()[axis])
                .unwrap_or(std::cmp::Ordering::Equal)
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

    fn find_nearest(&self, p: Vec3, mut max_dist: f32) -> Option<RayHit> {
        if self.aabb().sq_dist_to_point(p) > max_dist * max_dist {
            return None;
        }

        match self {
            BVHNode::Leaf { triangles, .. } => {
                let mut best_hit: Option<RayHit> = None;
                for tri in triangles {
                    let (pt, norm, dist) = tri.nearest_point(p);
                    if dist < max_dist {
                        max_dist = dist;
                        best_hit = Some(RayHit {
                            point: pt,
                            normal: norm,
                            face_index: tri.index,
                            distance: dist,
                        });
                    }
                }
                best_hit
            }
            BVHNode::Internal { left, right, .. } => {
                let left_sq = left.aabb().sq_dist_to_point(p);
                let right_sq = right.aabb().sq_dist_to_point(p);

                let (first, second) = if left_sq < right_sq {
                    (left, right)
                } else {
                    (right, left)
                };

                let mut best_hit = first.find_nearest(p, max_dist);
                if let Some(ref hit) = best_hit {
                    max_dist = hit.distance;
                }

                let hit_second = second.find_nearest(p, max_dist);
                if let Some(r) = hit_second {
                    best_hit = Some(r);
                }

                best_hit
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn from_polygons(verts: &[Vec3], polys: &[[usize; 3]]) -> Self {
        let triangles: Vec<Triangle> = polys
            .iter()
            .enumerate()
            .map(|(idx, poly)| {
                Triangle::new(
                    verts[poly[0]],
                    verts[poly[1]],
                    verts[poly[2]],
                    idx,
                )
            })
            .collect();

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

    #[allow(non_snake_case)]
    pub fn FromPolygons(verts: &[Vec3], polys: &[[usize; 3]]) -> Self {
        Self::from_polygons(verts, polys)
    }

    pub fn ray_cast(&self, origin: Vec3, direction: Vec3, max_dist: f32) -> Option<RayHit> {
        let ray = Ray::new(origin, direction);
        let dist = if max_dist <= 0.0 { f32::INFINITY } else { max_dist };
        self.root.as_ref().and_then(|r| r.ray_cast(&ray, dist))
    }

    pub fn find_nearest(&self, point: Vec3, max_dist: f32) -> Option<RayHit> {
        let dist = if max_dist <= 0.0 { f32::INFINITY } else { max_dist };
        self.root.as_ref().and_then(|r| r.find_nearest(point, dist))
    }
}
