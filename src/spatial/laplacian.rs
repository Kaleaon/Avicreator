use glam::Vec3;
use rayon::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

/// Spatial adjacency structure precomputing cotangent edge weights and mesh topology.
/// Caches adjacency structures to ensure zero heap allocations during per-frame relaxation loops.
#[derive(Debug, Clone)]
pub struct CotangentLaplacianCache {
    pub num_vertices: usize,
    pub offsets: Vec<usize>,
    pub neighbors: Vec<usize>,
    pub weights: Vec<f32>,
    pub weight_sums: Vec<f32>,
}

thread_local! {
    static SCRATCH_LAPLACIAN_STEP: RefCell<Vec<Vec3>> = const { RefCell::new(Vec::new()) };
}

impl CotangentLaplacianCache {
    /// Constructs cotangent weight matrices over mesh topology.
    pub fn build(vertices: &[Vec3], polygons: &[[usize; 3]]) -> Self {
        let num_vertices = vertices.len();
        if num_vertices == 0 || polygons.is_empty() {
            return Self {
                num_vertices,
                offsets: vec![0; num_vertices + 1],
                neighbors: Vec::new(),
                weights: Vec::new(),
                weight_sums: vec![0.0; num_vertices],
            };
        }

        let mut edge_weights: HashMap<(usize, usize), f32> = HashMap::new();

        for poly in polygons {
            let i0 = poly[0];
            let i1 = poly[1];
            let i2 = poly[2];

            if i0 >= num_vertices || i1 >= num_vertices || i2 >= num_vertices {
                continue;
            }

            let p0 = vertices[i0];
            let p1 = vertices[i1];
            let p2 = vertices[i2];

            // Edge (i0, i1) opposite i2
            let u0 = p0 - p2;
            let v0 = p1 - p2;
            let cross0 = u0.cross(v0);
            let len0 = cross0.length();
            let cot0 = if len0 > 1e-8 { u0.dot(v0) / len0 } else { 0.0 };
            let w0 = (0.5 * cot0).max(1e-4);

            // Edge (i1, i2) opposite i0
            let u1 = p1 - p0;
            let v1 = p2 - p0;
            let cross1 = u1.cross(v1);
            let len1 = cross1.length();
            let cot1 = if len1 > 1e-8 { u1.dot(v1) / len1 } else { 0.0 };
            let w1 = (0.5 * cot1).max(1e-4);

            // Edge (i2, i0) opposite i1
            let u2 = p2 - p1;
            let v2 = p0 - p1;
            let cross2 = u2.cross(v2);
            let len2 = cross2.length();
            let cot2 = if len2 > 1e-8 { u2.dot(v2) / len2 } else { 0.0 };
            let w2 = (0.5 * cot2).max(1e-4);

            let e0 = if i0 < i1 { (i0, i1) } else { (i1, i0) };
            *edge_weights.entry(e0).or_insert(0.0) += w0;

            let e1 = if i1 < i2 { (i1, i2) } else { (i2, i1) };
            *edge_weights.entry(e1).or_insert(0.0) += w1;

            let e2 = if i2 < i0 { (i2, i0) } else { (i0, i2) };
            *edge_weights.entry(e2).or_insert(0.0) += w2;
        }

        // Build adjacency lists per vertex
        let mut adj: Vec<Vec<(usize, f32)>> = vec![Vec::new(); num_vertices];
        for ((u, v), w) in edge_weights {
            adj[u].push((v, w));
            adj[v].push((u, w));
        }

        let mut offsets = Vec::with_capacity(num_vertices + 1);
        let mut neighbors = Vec::new();
        let mut weights = Vec::new();
        let mut weight_sums = Vec::with_capacity(num_vertices);

        offsets.push(0);
        for list in adj {
            let mut sum_w = 0.0f32;
            for (nbr, w) in list {
                neighbors.push(nbr);
                weights.push(w);
                sum_w += w;
            }
            weight_sums.push(sum_w);
            offsets.push(neighbors.len());
        }

        Self {
            num_vertices,
            offsets,
            neighbors,
            weights,
            weight_sums,
        }
    }

    /// Computes Laplacian displacement vectors L_i for all vertices using thread-local zero-alloc scratch buffer.
    pub fn compute_laplacian_vectors(&self, positions: &[Vec3], laplacian_out: &mut [Vec3]) {
        if positions.len() < self.num_vertices || laplacian_out.len() < self.num_vertices {
            return;
        }

        let offsets = &self.offsets;
        let neighbors = &self.neighbors;
        let weights = &self.weights;
        let weight_sums = &self.weight_sums;

        laplacian_out[..self.num_vertices]
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, out)| {
                let sum_w = unsafe { *weight_sums.get_unchecked(i) };
                if sum_w <= 1e-8 {
                    *out = Vec3::ZERO;
                    return;
                }

                let start = unsafe { *offsets.get_unchecked(i) };
                let end = unsafe { *offsets.get_unchecked(i + 1) };
                let p_i = unsafe { *positions.get_unchecked(i) };
                let mut acc = Vec3::ZERO;

                for k in start..end {
                    let j = unsafe { *neighbors.get_unchecked(k) };
                    let w = unsafe { *weights.get_unchecked(k) };
                    acc += (unsafe { *positions.get_unchecked(j) } - p_i) * w;
                }

                *out = acc / sum_w;
            });
    }

    /// Helper to get a thread-local zero-alloc scratch buffer for laplacian step calculations.
    pub fn with_scratch_buffer<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut [Vec3]) -> R,
    {
        SCRATCH_LAPLACIAN_STEP.with(|cell| {
            let mut vec = cell.borrow_mut();
            if vec.len() < self.num_vertices {
                vec.resize(self.num_vertices, Vec3::ZERO);
            } else {
                vec[..self.num_vertices].fill(Vec3::ZERO);
            }
            f(&mut vec[..self.num_vertices])
        })
    }
}

/// Computes the total enclosed 3D mesh volume using signed tetrahedral decomposition.
pub fn calculate_enclosed_volume(vertices: &[Vec3], polygons: &[[usize; 3]]) -> f32 {
    let (vol, _) = calculate_volume_and_center(vertices, polygons);
    vol
}

/// Computes the volumetric center (3D centroid of enclosed volume).
pub fn calculate_volumetric_center(vertices: &[Vec3], polygons: &[[usize; 3]]) -> Vec3 {
    let (_, center) = calculate_volume_and_center(vertices, polygons);
    center
}

/// Computes total enclosed 3D volume and volumetric center in a single parallel pass over polygons.
pub fn calculate_volume_and_center(vertices: &[Vec3], polygons: &[[usize; 3]]) -> (f32, Vec3) {
    if vertices.is_empty() || polygons.is_empty() {
        return (0.0, Vec3::ZERO);
    }

    let (signed_vol_sum, weighted_center_sum) = polygons
        .par_iter()
        .fold(
            || (0.0f64, Vec3::ZERO),
            |(acc_vol, acc_center), poly| {
                let i0 = poly[0];
                let i1 = poly[1];
                let i2 = poly[2];

                if i0 < vertices.len() && i1 < vertices.len() && i2 < vertices.len() {
                    let p0 = unsafe { *vertices.get_unchecked(i0) };
                    let p1 = unsafe { *vertices.get_unchecked(i1) };
                    let p2 = unsafe { *vertices.get_unchecked(i2) };

                    let cross = p0.cross(p1);
                    let tet_vol_raw = cross.dot(p2) as f64;
                    let tet_vol_abs = (tet_vol_raw / 6.0).abs() as f32;

                    let tet_center = (p0 + p1 + p2) * 0.25;
                    (acc_vol + tet_vol_raw, acc_center + tet_center * tet_vol_abs)
                } else {
                    (acc_vol, acc_center)
                }
            },
        )
        .reduce(
            || (0.0f64, Vec3::ZERO),
            |(v1, c1), (v2, c2)| (v1 + v2, c1 + c2),
        );

    let total_vol = (signed_vol_sum / 6.0).abs() as f32;
    let center = if total_vol > 1e-8 {
        weighted_center_sum / total_vol
    } else {
        let sum: Vec3 = vertices.par_iter().copied().sum();
        sum / (vertices.len() as f32)
    };

    (total_vol, center)
}

/// Computes vertex surface normals from polygon topology.
pub fn calculate_surface_normals(vertices: &[Vec3], polygons: &[[usize; 3]]) -> Vec<Vec3> {
    if vertices.is_empty() {
        return Vec::new();
    }

    let accumulated: Vec<Vec3> = polygons
        .par_iter()
        .fold(
            || vec![Vec3::ZERO; vertices.len()],
            |mut local_acc, poly| {
                let i0 = poly[0];
                let i1 = poly[1];
                let i2 = poly[2];

                if i0 < vertices.len() && i1 < vertices.len() && i2 < vertices.len() {
                    let p0 = unsafe { *vertices.get_unchecked(i0) };
                    let p1 = unsafe { *vertices.get_unchecked(i1) };
                    let p2 = unsafe { *vertices.get_unchecked(i2) };

                    let e1 = p1 - p0;
                    let e2 = p2 - p0;
                    let face_normal = e1.cross(e2);

                    unsafe {
                        *local_acc.get_unchecked_mut(i0) += face_normal;
                        *local_acc.get_unchecked_mut(i1) += face_normal;
                        *local_acc.get_unchecked_mut(i2) += face_normal;
                    }
                }
                local_acc
            },
        )
        .reduce(
            || vec![Vec3::ZERO; vertices.len()],
            |mut a, b| {
                for (a_elem, b_elem) in a.iter_mut().zip(b.iter()) {
                    *a_elem += *b_elem;
                }
                a
            },
        );

    accumulated
        .into_par_iter()
        .map(|n| if n.length_squared() > 1e-8 { n.normalize() } else { Vec3::Y })
        .collect()
}
