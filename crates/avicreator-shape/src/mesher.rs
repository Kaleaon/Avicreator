//! Converts a compiled shape into a skinned triangle mesh.
//!
//! 1. **Extract** – naive surface nets (one vertex per cell that straddles
//!    the surface, one quad per sign-changing grid edge) give smooth,
//!    watertight meshes with no lookup tables. Storage is sparse: only 8³-cell
//!    blocks near the surface are kept, so memory follows surface area and
//!    fine grids (thin fingers, lips, claws) stay affordable.
//! 2. **Simplify** (optional) – quadric edge collapse to a triangle budget,
//!    keeping detail where the surface curves.
//! 3. **Skin** – normals and bone weights are sampled from the continuous
//!    fields at the final vertices.
//!
//! Two accelerations leave the surface unchanged:
//! * **Primitive culling** – each 32³ super-block and 8³ block evaluates only
//!   primitives whose bounds, padded by the blend radii, reach it.
//! * **Narrow band** – a region (32³, 8³, then 4³ cells) whose centre is
//!   farther from the surface than its own extent is never sampled in detail.

use std::collections::HashMap;

use avicreator_core::{normalize_vertex_weights, Mesh, Vec3};

use crate::compile::CompiledShape;
use crate::sdf::Aabb;
use crate::simplify::simplify;

/// Wall-clock timing that compiles to a no-op on wasm32, where
/// `std::time::Instant` is unavailable.
struct Stopwatch(#[cfg(not(target_arch = "wasm32"))] std::time::Instant);

impl Stopwatch {
    fn start() -> Self {
        Stopwatch(
            #[cfg(not(target_arch = "wasm32"))]
            std::time::Instant::now(),
        )
    }

    fn millis(&self) -> f64 {
        #[cfg(not(target_arch = "wasm32"))]
        return self.0.elapsed().as_secs_f64() * 1000.0;
        #[cfg(target_arch = "wasm32")]
        return 0.0;
    }
}

/// Cells per super-block edge (coarse culling and skipping).
const SUPER: usize = 32;
/// Cells per block edge (storage unit and primitive culling).
const BLOCK: usize = 8;
/// Cells per sub-block edge (finest narrow-band test).
const SUB: usize = 4;
/// Grid points per block edge.
const PTS: usize = BLOCK + 1;
/// Safety factor on the distance bound used to skip regions. Round cones and
/// slabs are exact distances and smooth min/max are convex blends, so only
/// the ellipsoid bound is approximate.
const LIPSCHITZ: f32 = 1.25;
pub const MAX_RESOLUTION: u32 = 2048;

#[derive(Debug, Clone, Copy)]
pub struct MeshOptions {
    /// Grid cells along the longest bounding-box axis.
    pub resolution: u32,
    /// Skip far regions and cull distant primitives (same surface, faster).
    pub accelerate: bool,
    /// Newton steps that pull vertices onto the exact zero set.
    pub projection_steps: u32,
    /// Simplify to at most this many triangles.
    pub triangles: Option<usize>,
    /// Sample normals and bone weights (skip for geometry-only uses).
    pub skin: bool,
}

impl MeshOptions {
    pub fn new(resolution: u32) -> Self {
        Self { resolution, accelerate: true, projection_steps: 2, triangles: None, skin: true }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MeshStats {
    pub grid: [usize; 3],
    pub cell_size: f32,
    pub samples_evaluated: usize,
    pub samples_total: usize,
    /// Counted in 4x4x4-cell sub-blocks.
    pub blocks_skipped: usize,
    pub blocks_total: usize,
    /// Triangles straight out of extraction, before simplification.
    pub raw_triangles: usize,
    pub vertices: usize,
    pub triangles: usize,
    /// Total and simplification time; zero on wasm32, where no clock is available.
    pub millis: f64,
    pub simplify_millis: f64,
}

pub struct MeshResult {
    pub mesh: Mesh,
    pub stats: MeshStats,
}

struct Grid {
    origin: Vec3,
    h: f32,
}

impl Grid {
    fn point(&self, p: [usize; 3]) -> Vec3 {
        self.origin.add(Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32).scale(self.h))
    }

    fn span(&self, lo: [usize; 3], hi: [usize; 3]) -> Aabb {
        Aabb { min: self.point(lo), max: self.point(hi) }
    }
}

/// Samples and cell vertices of one 8³-cell block near the surface.
struct Block {
    lo: [usize; 3],
    active: Vec<u32>,
    values: Vec<f32>,
    exact: Vec<bool>,
    cell_vertex: Vec<u32>,
    live: Vec<([usize; 3], [usize; 3])>,
}

impl Block {
    fn pidx(&self, p: [usize; 3]) -> usize {
        ((p[2] - self.lo[2]) * PTS + (p[1] - self.lo[1])) * PTS + (p[0] - self.lo[0])
    }

    fn cidx(&self, c: [usize; 3]) -> usize {
        ((c[2] - self.lo[2]) * BLOCK + (c[1] - self.lo[1])) * BLOCK + (c[0] - self.lo[0])
    }

    fn fill(&mut self, lo: [usize; 3], hi: [usize; 3], d: f32) {
        for k in lo[2]..=hi[2] {
            for j in lo[1]..=hi[1] {
                for i in lo[0]..=hi[0] {
                    let idx = self.pidx([i, j, k]);
                    if !self.exact[idx] {
                        self.values[idx] = d;
                    }
                }
            }
        }
    }
}

fn range_min(a: [usize; 3], b: [usize; 3]) -> [usize; 3] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]
}

pub fn mesh(shape: &CompiledShape, opts: MeshOptions) -> MeshResult {
    let clock = Stopwatch::start();
    let res = opts.resolution.clamp(8, MAX_RESOLUTION) as f32;
    let size = shape.bounds.size();
    let longest = size.x.max(size.y).max(size.z).max(1e-3);
    let h = longest / res;
    // Pad by two cells so the surface never touches the grid boundary.
    let origin = shape.bounds.min.sub(Vec3::new(2.0 * h, 2.0 * h, 2.0 * h));
    let n = [
        (size.x / h).ceil() as usize + 5,
        (size.y / h).ceil() as usize + 5,
        (size.z / h).ceil() as usize + 5,
    ];
    let grid = Grid { origin, h };
    let last = [n[0] - 1, n[1] - 1, n[2] - 1];
    let mut stats = MeshStats { grid: n, cell_size: h, samples_total: n[0] * n[1] * n[2], ..Default::default() };

    let all: Vec<u32> = (0..shape.prims.len() as u32).collect();
    let max_blend = shape.parts.iter().map(|p| p.blend + p.smooth).fold(0.0, f32::max);
    let cull_margin = 2.0 * max_blend + 4.0 * h;
    let cull = |from: &[u32], region: &Aabb| -> Vec<u32> {
        if !opts.accelerate {
            return from.to_vec();
        }
        let reach = region.inflate(cull_margin);
        from.iter().copied().filter(|&i| shape.prims[i as usize].bounds.intersects(&reach)).collect()
    };
    let far = |active: &[u32], region: &Aabb| -> bool {
        if !opts.accelerate {
            return false;
        }
        if active.is_empty() {
            return true;
        }
        let d = shape.distance_subset(region.center(), active);
        // The 2h margin keeps every point of a skipped region farther from the
        // surface than any sign-changing edge can be, so no quad loses a cell.
        d.abs() > LIPSCHITZ * region.size().length() * 0.5 + 2.0 * h
    };
    let subs_in = |lo: [usize; 3], hi: [usize; 3], step: usize| -> usize {
        (0..3).map(|a| (hi[a] - lo[a]).div_ceil(step)).product()
    };

    let mut blocks: Vec<Block> = Vec::new();
    let mut block_at: HashMap<[usize; 3], usize> = HashMap::new();
    let supers = [last[0].div_ceil(SUPER), last[1].div_ceil(SUPER), last[2].div_ceil(SUPER)];
    for sk in 0..supers[2] {
        for sj in 0..supers[1] {
            for si in 0..supers[0] {
                let slo = [si * SUPER, sj * SUPER, sk * SUPER];
                let shi = range_min([slo[0] + SUPER, slo[1] + SUPER, slo[2] + SUPER], last);
                let sregion = grid.span(slo, shi);
                let sactive = cull(&all, &sregion);
                let sub_count = subs_in(slo, shi, SUB);
                stats.blocks_total += sub_count;
                if far(&sactive, &sregion) {
                    stats.blocks_skipped += sub_count;
                    continue;
                }
                for bk in (slo[2]..shi[2]).step_by(BLOCK) {
                    for bj in (slo[1]..shi[1]).step_by(BLOCK) {
                        for bi in (slo[0]..shi[0]).step_by(BLOCK) {
                            let lo = [bi, bj, bk];
                            let hi = range_min([bi + BLOCK, bj + BLOCK, bk + BLOCK], shi);
                            let region = grid.span(lo, hi);
                            let active = cull(&sactive, &region);
                            if far(&active, &region) {
                                stats.blocks_skipped += subs_in(lo, hi, SUB);
                                continue;
                            }
                            let mut block = Block {
                                lo,
                                active,
                                values: vec![f32::NAN; PTS * PTS * PTS],
                                exact: vec![false; PTS * PTS * PTS],
                                cell_vertex: vec![u32::MAX; BLOCK * BLOCK * BLOCK],
                                live: Vec::new(),
                            };
                            for k in (lo[2]..hi[2]).step_by(SUB) {
                                for j in (lo[1]..hi[1]).step_by(SUB) {
                                    for i in (lo[0]..hi[0]).step_by(SUB) {
                                        let a = [i, j, k];
                                        let b = range_min([i + SUB, j + SUB, k + SUB], hi);
                                        let sub = grid.span(a, b);
                                        if far(&block.active, &sub) {
                                            stats.blocks_skipped += 1;
                                            let d = shape.distance_subset(sub.center(), &block.active);
                                            block.fill(a, b, d);
                                            continue;
                                        }
                                        for z in a[2]..=b[2] {
                                            for y in a[1]..=b[1] {
                                                for x in a[0]..=b[0] {
                                                    let idx = block.pidx([x, y, z]);
                                                    if !block.exact[idx] {
                                                        block.values[idx] = shape.distance_subset(grid.point([x, y, z]), &block.active);
                                                        block.exact[idx] = true;
                                                        stats.samples_evaluated += 1;
                                                    }
                                                }
                                            }
                                        }
                                        block.live.push((a, b));
                                    }
                                }
                            }
                            if !block.live.is_empty() {
                                block_at.insert([bi / BLOCK, bj / BLOCK, bk / BLOCK], blocks.len());
                                blocks.push(block);
                            }
                        }
                    }
                }
            }
        }
    }

    // One vertex per straddling cell, placed at the mean of its edge crossings.
    const CORNERS: [[usize; 3]; 8] =
        [[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0], [0, 0, 1], [1, 0, 1], [0, 1, 1], [1, 1, 1]];
    const EDGES: [[usize; 2]; 12] =
        [[0, 1], [2, 3], [4, 5], [6, 7], [0, 2], [1, 3], [4, 6], [5, 7], [0, 4], [1, 5], [2, 6], [3, 7]];
    let mut positions = Vec::new();
    let mut straddling: Vec<(usize, [usize; 3])> = Vec::new();
    for (bi, block) in blocks.iter_mut().enumerate() {
        for (a, b) in block.live.clone() {
            for k in a[2]..b[2] {
                for j in a[1]..b[1] {
                    for i in a[0]..b[0] {
                        let mut v = [0.0f32; 8];
                        let mut inside = 0;
                        for (c, off) in CORNERS.iter().enumerate() {
                            v[c] = block.values[block.pidx([i + off[0], j + off[1], k + off[2]])];
                            if v[c] < 0.0 {
                                inside += 1;
                            }
                        }
                        if inside == 0 || inside == 8 {
                            continue;
                        }
                        let mut sum = Vec3::ZERO;
                        let mut count = 0.0;
                        for [ea, eb] in EDGES {
                            if (v[ea] < 0.0) != (v[eb] < 0.0) {
                                let t = v[ea] / (v[ea] - v[eb]);
                                let pa = Vec3::new(CORNERS[ea][0] as f32, CORNERS[ea][1] as f32, CORNERS[ea][2] as f32);
                                let pb = Vec3::new(CORNERS[eb][0] as f32, CORNERS[eb][1] as f32, CORNERS[eb][2] as f32);
                                sum = sum.add(pa.lerp(pb, t));
                                count += 1.0;
                            }
                        }
                        let cell = block.cidx([i, j, k]);
                        block.cell_vertex[cell] = positions.len() as u32;
                        positions.push(grid.point([i, j, k]).add(sum.scale(h / count)));
                        straddling.push((bi, [i, j, k]));
                    }
                }
            }
        }
    }

    // Pull vertices onto the zero set, staying within the cell neighbourhood.
    for _ in 0..opts.projection_steps {
        for (vi, p) in positions.iter_mut().enumerate() {
            let (bi, cell) = straddling[vi];
            let active = &blocks[bi].active;
            let d = shape.distance_subset(*p, active);
            let g = shape.gradient_subset(*p, active, h * 0.25);
            let g2 = g.length_squared();
            if g2 < 1e-8 {
                continue;
            }
            let moved = p.sub(g.scale(d / g2));
            let rel = moved.sub(grid.point(cell).sub(Vec3::new(0.5 * h, 0.5 * h, 0.5 * h)));
            if [rel.x, rel.y, rel.z].iter().all(|&c| (0.0..=2.0 * h).contains(&c)) {
                *p = moved;
            }
        }
    }

    // One quad per sign-changing edge from each cell's minimum corner,
    // joining the four cells around that edge, wound inside -> outside.
    let vertex_of = |c: [usize; 3]| -> u32 {
        block_at
            .get(&[c[0] / BLOCK, c[1] / BLOCK, c[2] / BLOCK])
            .map_or(u32::MAX, |&b| blocks[b].cell_vertex[blocks[b].cidx(c)])
    };
    let mut indices: Vec<u32> = Vec::new();
    for &(bi, base) in &straddling {
        let block = &blocks[bi];
        let v0 = block.values[block.pidx(base)];
        for axis in 0..3 {
            let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
            if base[b] == 0 || base[c] == 0 {
                continue;
            }
            let mut end = base;
            end[axis] += 1;
            let v1 = block.values[block.pidx(end)];
            if (v0 < 0.0) == (v1 < 0.0) {
                continue;
            }
            let mut cb = base;
            cb[b] -= 1;
            let mut cbc = cb;
            cbc[c] -= 1;
            let mut cc = base;
            cc[c] -= 1;
            let quad = [base, cb, cbc, cc].map(vertex_of);
            if quad.contains(&u32::MAX) {
                continue;
            }
            let [q0, q1, q2, q3] = if v0 < 0.0 { quad } else { [quad[0], quad[3], quad[2], quad[1]] };
            let p = |q: u32| positions[q as usize];
            // Split along the shorter diagonal for better-shaped triangles.
            if p(q0).sub(p(q2)).length_squared() <= p(q1).sub(p(q3)).length_squared() {
                indices.extend_from_slice(&[q0, q1, q2, q0, q2, q3]);
            } else {
                indices.extend_from_slice(&[q0, q1, q3, q1, q2, q3]);
            }
        }
    }
    stats.raw_triangles = indices.len() / 3;

    let (mut positions, indices) = match opts.triangles {
        Some(target) if target < indices.len() / 3 => {
            let timer = Stopwatch::start();
            let (mut p, i) = simplify(&positions, &indices, target);
            stats.simplify_millis = timer.millis();
            // Simplified vertices sit near the surface; settle them onto it.
            for v in p.iter_mut() {
                let d = shape.distance(*v);
                let g = shape.gradient(*v, h * 0.25);
                let g2 = g.length_squared();
                if g2 > 1e-8 && d.abs() < 2.0 * h {
                    *v = v.sub(g.scale(d / g2));
                }
            }
            (p, i)
        }
        _ => (positions, indices),
    };
    positions.shrink_to_fit();

    if !opts.skin {
        stats.vertices = positions.len();
        stats.triangles = indices.len() / 3;
        stats.millis = clock.millis();
        let count = positions.len();
        return MeshResult {
            mesh: Mesh::new(positions, indices),
            stats: MeshStats { vertices: count, ..stats },
        };
    }

    let normals: Vec<Vec3> = positions
        .iter()
        .map(|&p| {
            let g = shape.gradient(p, h * 0.25).normalize();
            if g.length_squared() > 0.5 { g } else { Vec3::Y }
        })
        .collect();

    let mut bone_indices = Vec::with_capacity(positions.len());
    let mut bone_weights = Vec::with_capacity(positions.len());
    for &p in &positions {
        let mut idx = [0u32; 4];
        let mut w = [0.0f32; 4];
        for (slot, (bone, weight)) in shape.weights(p).into_iter().enumerate() {
            idx[slot] = bone as u32;
            w[slot] = weight;
        }
        bone_indices.push(idx);
        bone_weights.push(w);
    }
    normalize_vertex_weights(&mut bone_weights, 1e-4);

    stats.vertices = positions.len();
    stats.triangles = indices.len() / 3;
    stats.millis = clock.millis();
    let count = positions.len();
    MeshResult {
        mesh: Mesh { positions, normals, uvs: vec![[0.0, 0.0]; count], indices, bone_weights, bone_indices },
        stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;
    use crate::program::ShapeProgram;

    fn shape(json: &str) -> CompiledShape {
        compile(&ShapeProgram::from_json(json).unwrap()).unwrap()
    }

    fn check_closed_and_outward(m: &Mesh, inside: Vec3) {
        let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
        for t in m.indices.chunks_exact(3) {
            let (a, b, c) = (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
            let normal = b.sub(a).cross(c.sub(a));
            let centroid = a.add(b).add(c).scale(1.0 / 3.0);
            assert!(normal.dot(centroid.sub(inside)) > 0.0, "inward-facing triangle");
            for (x, y) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *edges.entry((x.min(y), x.max(y))).or_default() += 1;
            }
        }
        assert!(edges.values().all(|&c| c == 2), "mesh must be watertight and manifold");
    }

    #[test]
    fn sphere_mesh_is_closed_outward_and_on_the_surface() {
        let s = shape(r#"{"parts":[{"kind":"blob","name":"ball","from":[0,1,0],"radii":0.5}]}"#);
        let out = mesh(&s, MeshOptions::new(32));
        let m = &out.mesh;
        assert!(m.vertex_count() > 500);
        let center = Vec3::new(0.0, 1.0, 0.0);
        for p in &m.positions {
            assert!((p.sub(center).length() - 0.5).abs() < 0.01, "vertex off the surface: {:?}", p);
        }
        check_closed_and_outward(m, center);
        // A single bone takes all the weight everywhere.
        assert!(m.bone_weights.iter().all(|w| w[0] == 1.0));
    }

    #[test]
    fn acceleration_does_not_change_the_mesh() {
        let s = shape(
            r#"{"settings":{"blend":0.08},"parts":[
                {"kind":"chain","name":"body","from":[0,1,0],"dir":[0,0,1],"length":1.2,"segments":3,"curl":20,"radius":[0.25,0.3,0.2]},
                {"kind":"chain","name":"leg","mirror":true,"attach":"body.0","at":0.5,"offset":[0.15,0,0],
                 "points":[[0.1,-0.4,0.1],[0.1,-0.8,-0.05]],"radius":[0.1,0.05]},
                {"kind":"blob","name":"eye","mirror":true,"op":"subtract","attach":"body.2","offset":[0.12,0.1,0.1],"radii":0.05}
            ]}"#,
        );
        let mut slow_opts = MeshOptions::new(40);
        slow_opts.accelerate = false;
        let slow = mesh(&s, slow_opts);
        let fast = mesh(&s, MeshOptions::new(40));
        assert!(fast.stats.samples_evaluated * 4 < slow.stats.samples_evaluated * 3, "{:?}", fast.stats);
        assert_eq!(fast.mesh.indices, slow.mesh.indices);
        assert_eq!(fast.mesh.positions.len(), slow.mesh.positions.len());
        for (a, b) in fast.mesh.positions.iter().zip(&slow.mesh.positions) {
            assert!(a.sub(*b).length() < 1e-4);
        }
    }

    #[test]
    fn fine_grids_are_sparse_and_simplify_to_a_budget() {
        let s = shape(r#"{"parts":[{"kind":"blob","name":"ball","from":[0,1,0],"radii":0.5}]}"#);
        let mut opts = MeshOptions::new(160);
        opts.triangles = Some(3000);
        let out = mesh(&s, opts);
        assert!(out.stats.raw_triangles > 100_000, "{:?}", out.stats);
        assert!(out.stats.samples_evaluated * 3 < out.stats.samples_total, "sampling should follow the surface: {:?}", out.stats);
        assert!(out.stats.triangles <= 3000 && out.stats.triangles > 2600);
        let center = Vec3::new(0.0, 1.0, 0.0);
        check_closed_and_outward(&out.mesh, center);
        for p in &out.mesh.positions {
            assert!((p.sub(center).length() - 0.5).abs() < 0.003);
        }
    }
}
