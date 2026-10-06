use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KDNode {
    point: Vec3,
    index: usize,
    axis: usize,
    left: Option<Box<KDNode>>,
    right: Option<Box<KDNode>>,
}

impl KDNode {
    fn build(mut points: Vec<(Vec3, usize)>, depth: usize) -> Option<Self> {
        if points.is_empty() {
            return None;
        }

        let axis = depth % 3;
        points.sort_by(|a, b| {
            a.0[axis]
                .partial_cmp(&b.0[axis])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mid = points.len() / 2;
        let (pt, idx) = points[mid];

        let left_pts = points[..mid].to_vec();
        let right_pts = points[mid + 1..].to_vec();

        Some(KDNode {
            point: pt,
            index: idx,
            axis,
            left: KDNode::build(left_pts, depth + 1).map(Box::new),
            right: KDNode::build(right_pts, depth + 1).map(Box::new),
        })
    }

    fn find_nearest(
        &self,
        target: Vec3,
        mut best: (Vec3, usize, f32),
    ) -> (Vec3, usize, f32) {
        let dist = self.point.distance(target);
        if dist < best.2 {
            best = (self.point, self.index, dist);
        }

        let axis = self.axis;
        let diff = target[axis] - self.point[axis];

        let (primary, secondary) = if diff <= 0.0 {
            (&self.left, &self.right)
        } else {
            (&self.right, &self.left)
        };

        if let Some(ref node) = primary {
            best = node.find_nearest(target, best);
        }

        if diff.abs() < best.2 {
            if let Some(ref node) = secondary {
                best = node.find_nearest(target, best);
            }
        }

        best
    }

    fn find_range(&self, target: Vec3, radius: f32, results: &mut Vec<(Vec3, usize, f32)>) {
        let dist = self.point.distance(target);
        if dist <= radius {
            results.push((self.point, self.index, dist));
        }

        let axis = self.axis;
        let diff = target[axis] - self.point[axis];

        if diff - radius <= 0.0 {
            if let Some(ref node) = self.left {
                node.find_range(target, radius, results);
            }
        }
        if diff + radius >= 0.0 {
            if let Some(ref node) = self.right {
                node.find_range(target, radius, results);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KDTree {
    root: Option<KDNode>,
    pub size: usize,
}

impl KDTree {
    pub fn new(size_hint: usize) -> Self {
        Self {
            root: None,
            size: size_hint,
        }
    }

    pub fn build(points: &[Vec3]) -> Self {
        let indexed_points: Vec<(Vec3, usize)> = points
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, p)| (p, i))
            .collect();

        let size = indexed_points.len();
        let root = KDNode::build(indexed_points, 0);
        Self { root, size }
    }

    pub fn find_nearest(&self, target: Vec3) -> Option<(Vec3, usize, f32)> {
        let root = self.root.as_ref()?;
        let initial_dist = root.point.distance(target);
        let best = (root.point, root.index, initial_dist);
        Some(root.find_nearest(target, best))
    }

    pub fn find_n_nearest(&self, target: Vec3, n: usize) -> Vec<(Vec3, usize, f32)> {
        if n == 0 || self.root.is_none() {
            return Vec::new();
        }

        let mut all_in_range = Vec::new();
        if let Some(ref root) = self.root {
            root.find_range(target, f32::INFINITY, &mut all_in_range);
        }
        all_in_range.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        all_in_range.truncate(n);
        all_in_range
    }

    pub fn find_range(&self, target: Vec3, radius: f32) -> Vec<(Vec3, usize, f32)> {
        let mut results = Vec::new();
        if let Some(ref root) = self.root {
            root.find_range(target, radius, &mut results);
        }
        results.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}
