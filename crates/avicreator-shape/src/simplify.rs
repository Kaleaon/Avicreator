//! Quadric-error mesh simplification (Garland & Heckbert edge collapse).
//!
//! Meshing at a resolution fine enough for fingers or lips over-tessellates
//! flat regions; collapsing the cheapest edges first keeps detail where the
//! surface curves and removes it where it does not. Collapses that would make
//! the mesh non-manifold (link condition) or fold a triangle over are
//! refused, so a closed, consistently wound input stays that way.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use avicreator_core::Vec3;

type Quadric = [f64; 10];

fn plane_quadric(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Quadric {
    let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len < 1e-20 {
        return [0.0; 10];
    }
    let area = 0.5 * len;
    let (x, y, z) = (n[0] / len, n[1] / len, n[2] / len);
    let d = -(x * a[0] + y * a[1] + z * a[2]);
    [x * x, x * y, x * z, x * d, y * y, y * z, y * d, z * z, z * d, d * d].map(|q| q * area)
}

fn add(a: &Quadric, b: &Quadric) -> Quadric {
    let mut out = *a;
    for i in 0..10 {
        out[i] += b[i];
    }
    out
}

fn error(q: &Quadric, p: [f64; 3]) -> f64 {
    let [x, y, z] = p;
    q[0] * x * x + 2.0 * q[1] * x * y + 2.0 * q[2] * x * z + 2.0 * q[3] * x + q[4] * y * y + 2.0 * q[5] * y * z
        + 2.0 * q[6] * y
        + q[7] * z * z
        + 2.0 * q[8] * z
        + q[9]
}

/// Position minimising the quadric, if the system is well conditioned.
fn optimum(q: &Quadric) -> Option<[f64; 3]> {
    let (a, b, c, d, e, f) = (q[0], q[1], q[2], q[4], q[5], q[7]);
    let det = a * (d * f - e * e) - b * (b * f - c * e) + c * (b * e - c * d);
    let scale = (a + d + f).abs().powi(3).max(1e-30);
    if det.abs() < 1e-9 * scale {
        return None;
    }
    let r = [-q[3], -q[6], -q[8]];
    let x = (r[0] * (d * f - e * e) - b * (r[1] * f - e * r[2]) + c * (r[1] * e - d * r[2])) / det;
    let y = (a * (r[1] * f - e * r[2]) - r[0] * (b * f - c * e) + c * (b * r[2] - r[1] * c)) / det;
    let z = (a * (d * r[2] - r[1] * e) - b * (b * r[2] - r[1] * c) + r[0] * (b * e - c * d)) / det;
    Some([x, y, z])
}

/// A queued edge collapse. `stamp` mixes both endpoints' versions; an entry
/// whose endpoints changed since it was queued is skipped. A rare stale entry
/// that slips through is harmless because placement is recomputed on pop.
#[derive(PartialEq, Clone, Copy)]
struct Candidate {
    cost: f32,
    u: u32,
    v: u32,
    stamp: u32,
}

impl Eq for Candidate {}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap on cost.
        other.cost.total_cmp(&self.cost).then_with(|| (other.u, other.v).cmp(&(self.u, self.v)))
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct State {
    pos: Vec<[f64; 3]>,
    quadric: Vec<Quadric>,
    faces: Vec<[u32; 3]>,
    face_alive: Vec<bool>,
    vert_faces: Vec<Vec<u32>>,
    stamp: Vec<u32>,
    alive: Vec<bool>,
    /// Vertices on non-manifold edges or pinch points, where thin features
    /// meet. Collapsing them could zip separate surfaces together.
    locked: Vec<bool>,
    scratch_a: Vec<u32>,
    scratch_b: Vec<u32>,
}

impl State {
    fn stamp_of(&self, u: u32, v: u32) -> u32 {
        self.stamp[u as usize].wrapping_mul(0x9E37_79B1) ^ self.stamp[v as usize]
    }

    fn live_faces(&self, v: u32) -> impl Iterator<Item = u32> + '_ {
        self.vert_faces[v as usize].iter().copied().filter(|&f| self.face_alive[f as usize])
    }

    fn neighbours_into(&self, v: u32, out: &mut Vec<u32>) {
        out.clear();
        for f in self.live_faces(v) {
            for &w in &self.faces[f as usize] {
                if w != v && !out.contains(&w) {
                    out.push(w);
                }
            }
        }
    }

    fn placement(&self, u: u32, v: u32) -> (f64, [f64; 3]) {
        let q = add(&self.quadric[u as usize], &self.quadric[v as usize]);
        let (a, b) = (self.pos[u as usize], self.pos[v as usize]);
        let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, (a[2] + b[2]) * 0.5];
        let len2 = (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>();
        let mut best = (error(&q, mid), mid);
        for p in [a, b] {
            let e = error(&q, p);
            if e < best.0 {
                best = (e, p);
            }
        }
        if let Some(p) = optimum(&q) {
            // Ignore optima that wander far from the edge (near-degenerate systems).
            let off = (0..3).map(|i| (p[i] - mid[i]).powi(2)).sum::<f64>();
            if off <= len2 {
                let e = error(&q, p);
                if e < best.0 {
                    best = (e, p);
                }
            }
        }
        (best.0.max(0.0), best.1)
    }

    fn push(&self, heap: &mut BinaryHeap<Candidate>, u: u32, v: u32) {
        let (cost, _) = self.placement(u, v);
        heap.push(Candidate { cost: cost as f32, u, v, stamp: self.stamp_of(u, v) });
    }

    fn valid(&self, c: &Candidate) -> bool {
        let (u, v) = (c.u as usize, c.v as usize);
        self.alive[u] && self.alive[v] && !self.locked[u] && !self.locked[v] && self.stamp_of(c.u, c.v) == c.stamp
    }

    fn normal(&self, f: &[u32; 3], moved: u32, to: [f64; 3]) -> [f64; 3] {
        let p = |i: u32| if i == moved { to } else { self.pos[i as usize] };
        let (a, b, c) = (p(f[0]), p(f[1]), p(f[2]));
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]
    }

    /// True if moving `moved` to `to` keeps every face around it (other than
    /// the two that vanish with edge `(u, v)`) facing the same way.
    fn keeps_orientation(&self, moved: u32, other: u32, to: [f64; 3]) -> bool {
        for f in self.live_faces(moved) {
            let face = self.faces[f as usize];
            if face.contains(&other) {
                continue;
            }
            let before = self.normal(&face, moved, self.pos[moved as usize]);
            let after = self.normal(&face, moved, to);
            let dot = before[0] * after[0] + before[1] * after[1] + before[2] * after[2];
            let lb = before.iter().map(|x| x * x).sum::<f64>().sqrt();
            let la = after.iter().map(|x| x * x).sum::<f64>().sqrt();
            if la < 1e-14 || dot < 0.2 * la * lb {
                return false;
            }
        }
        true
    }

    /// Collapses `v` into `u` at `to` if that keeps the mesh manifold and unfolded.
    fn try_collapse(&mut self, u: u32, v: u32, to: [f64; 3]) -> bool {
        let mut shared = [u32::MAX; 2];
        let mut count = 0;
        for f in self.live_faces(v) {
            if self.faces[f as usize].contains(&u) {
                if count < 2 {
                    shared[count] = f;
                }
                count += 1;
            }
        }
        if count != 2 {
            return false;
        }
        // Link condition: common neighbours must be exactly the two opposite vertices.
        let (mut nu, mut nv) = (std::mem::take(&mut self.scratch_a), std::mem::take(&mut self.scratch_b));
        self.neighbours_into(u, &mut nu);
        self.neighbours_into(v, &mut nv);
        let common = nv.iter().filter(|w| nu.contains(w)).count();
        self.scratch_a = nu;
        self.scratch_b = nv;
        if common != 2 || !self.keeps_orientation(u, v, to) || !self.keeps_orientation(v, u, to) {
            return false;
        }

        for f in shared {
            self.face_alive[f as usize] = false;
        }
        let moved = std::mem::take(&mut self.vert_faces[v as usize]);
        for &f in &moved {
            if !self.face_alive[f as usize] {
                continue;
            }
            for w in self.faces[f as usize].iter_mut() {
                if *w == v {
                    *w = u;
                }
            }
            self.vert_faces[u as usize].push(f);
        }
        self.alive[v as usize] = false;
        self.pos[u as usize] = to;
        self.quadric[u as usize] = add(&self.quadric[u as usize], &self.quadric[v as usize]);
        self.stamp[u as usize] = self.stamp[u as usize].wrapping_add(1);
        let face_alive = &self.face_alive;
        self.vert_faces[u as usize].retain(|&f| face_alive[f as usize]);
        true
    }
}

/// Locks the vertices of every edge not shared by exactly two faces, and of
/// every vertex whose faces form more than one fan (two surfaces pinched
/// together at a point).
fn lock_non_manifold(s: &mut State) {
    let mut fan: Vec<(u32, u32)> = Vec::new();
    for v in 0..s.pos.len() {
        // Edges (v, w) of v's faces: each must appear exactly twice, and the
        // faces must connect into one fan through them.
        fan.clear();
        for &f in &s.vert_faces[v] {
            for &w in s.faces[f as usize].iter().filter(|&&w| w as usize != v) {
                fan.push((w, f));
            }
        }
        fan.sort_unstable();
        let mut manifold = true;
        let mut i = 0;
        while i < fan.len() {
            let j = (i..fan.len()).find(|&j| fan[j].0 != fan[i].0).unwrap_or(fan.len());
            if j - i != 2 {
                manifold = false;
                break;
            }
            i = j;
        }
        if manifold && !s.vert_faces[v].is_empty() {
            // Walk the fan from one face across shared edges; it must reach all.
            let faces = &s.vert_faces[v];
            let mut seen = vec![false; faces.len()];
            let mut stack = vec![0usize];
            seen[0] = true;
            let mut reached = 1;
            while let Some(k) = stack.pop() {
                let fk = faces[k];
                for &(w, f) in &fan {
                    if f != fk {
                        continue;
                    }
                    for &(w2, f2) in &fan {
                        if w2 == w && f2 != fk {
                            if let Some(n) = faces.iter().position(|&x| x == f2) {
                                if !seen[n] {
                                    seen[n] = true;
                                    reached += 1;
                                    stack.push(n);
                                }
                            }
                        }
                    }
                }
            }
            manifold = reached == faces.len();
        }
        if !manifold {
            s.locked[v] = true;
        }
    }
}

/// Simplifies a closed triangle mesh to at most `target` triangles (when
/// possible) and returns the compacted positions and indices.
pub fn simplify(positions: &[Vec3], indices: &[u32], target: usize) -> (Vec<Vec3>, Vec<u32>) {
    let face_count = indices.len() / 3;
    if face_count <= target {
        return (positions.to_vec(), indices.to_vec());
    }
    let mut s = State {
        pos: positions.iter().map(|p| [p.x as f64, p.y as f64, p.z as f64]).collect(),
        quadric: vec![[0.0; 10]; positions.len()],
        faces: indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect(),
        face_alive: vec![true; face_count],
        vert_faces: vec![Vec::new(); positions.len()],
        stamp: vec![0; positions.len()],
        alive: vec![true; positions.len()],
        locked: vec![false; positions.len()],
        scratch_a: Vec::new(),
        scratch_b: Vec::new(),
    };
    for (f, face) in s.faces.iter().enumerate() {
        let q = plane_quadric(s.pos[face[0] as usize], s.pos[face[1] as usize], s.pos[face[2] as usize]);
        for &v in face {
            s.quadric[v as usize] = add(&s.quadric[v as usize], &q);
            s.vert_faces[v as usize].push(f as u32);
        }
    }

    lock_non_manifold(&mut s);

    let mut heap = BinaryHeap::new();
    for face in &s.faces {
        for (a, b) in [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])] {
            if a < b {
                s.push(&mut heap, a, b);
            }
        }
    }

    let mut live_faces = face_count;
    let mut around = Vec::new();
    while live_faces > target {
        // Lazy deletion leaves stale entries; drop them once they dominate so
        // the heap stays proportional to the live edges.
        if heap.len() > 4 * live_faces + 4096 {
            heap = heap.into_iter().filter(|c| s.valid(c)).collect();
        }
        let Some(c) = heap.pop() else { break };
        if !s.valid(&c) {
            continue;
        }
        let (_, to) = s.placement(c.u, c.v);
        if s.try_collapse(c.u, c.v, to) {
            live_faces -= 2;
            s.neighbours_into(c.u, &mut around);
            for &w in &around {
                s.push(&mut heap, c.u.min(w), c.u.max(w));
            }
        }
    }

    let mut remap = vec![u32::MAX; positions.len()];
    let mut out_pos = Vec::new();
    let mut out_idx = Vec::with_capacity(live_faces * 3);
    for (f, face) in s.faces.iter().enumerate() {
        if !s.face_alive[f] {
            continue;
        }
        for &v in face {
            if remap[v as usize] == u32::MAX {
                remap[v as usize] = out_pos.len() as u32;
                let p = s.pos[v as usize];
                out_pos.push(Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32));
            }
            out_idx.push(remap[v as usize]);
        }
    }
    (out_pos, out_idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Subdivided octahedron projected to a sphere: closed and outward-wound.
    fn sphere(levels: u32) -> (Vec<Vec3>, Vec<u32>) {
        let mut pos = vec![Vec3::X, Vec3::X.scale(-1.0), Vec3::Y, Vec3::Y.scale(-1.0), Vec3::Z, Vec3::Z.scale(-1.0)];
        let mut tris: Vec<[u32; 3]> =
            vec![[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]];
        for _ in 0..levels {
            let mut mid: HashMap<(u32, u32), u32> = HashMap::new();
            let mut next = Vec::new();
            for t in &tris {
                let mut m = [0u32; 3];
                for e in 0..3 {
                    let (a, b) = (t[e].min(t[(e + 1) % 3]), t[e].max(t[(e + 1) % 3]));
                    m[e] = *mid.entry((a, b)).or_insert_with(|| {
                        pos.push(pos[a as usize].add(pos[b as usize]).normalize());
                        pos.len() as u32 - 1
                    });
                }
                next.extend([[t[0], m[0], m[2]], [m[0], t[1], m[1]], [m[2], m[1], t[2]], [m[0], m[1], m[2]]]);
            }
            tris = next;
        }
        (pos, tris.into_iter().flatten().collect())
    }

    #[test]
    fn surfaces_pinched_together_are_not_zipped() {
        // Two spheres touching at one shared vertex (+X of the first): a
        // pinch point where both fans meet.
        let (pos, idx) = sphere(4);
        let n = pos.len() as u32;
        let mut all = pos.clone();
        all.extend(pos.iter().map(|p| Vec3::new(2.0 - p.x, p.y, p.z)));
        let mut tris: Vec<u32> = idx.clone();
        for t in idx.chunks_exact(3) {
            // Mirrored, so reverse the winding; vertex 0 (+X) is shared.
            for &v in [t[0], t[2], t[1]].iter() {
                tris.push(if v == 0 { 0 } else { v + n });
            }
        }
        let (p, i) = simplify(&all, &tris, 600);
        assert!(i.len() / 3 <= 700, "{} triangles", i.len() / 3);
        for t in i.chunks_exact(3) {
            let xs = [p[t[0] as usize].x, p[t[1] as usize].x, p[t[2] as usize].x];
            let (lo, hi) = (xs.iter().cloned().fold(f32::MAX, f32::min), xs.iter().cloned().fold(f32::MIN, f32::max));
            assert!(lo > 0.99 || hi < 1.01, "triangle spans both spheres: {:?}", xs);
        }
        assert!(p.iter().any(|v| (v.sub(Vec3::X)).length() < 1e-6), "the pinch point stays put");
    }

    #[test]
    fn simplified_sphere_stays_closed_outward_and_round() {
        let (pos, idx) = sphere(5);
        assert_eq!(idx.len() / 3, 8192);
        let (p, i) = simplify(&pos, &idx, 1000);
        assert!(i.len() / 3 <= 1000 && i.len() / 3 >= 900, "{} triangles", i.len() / 3);
        let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
        for t in i.chunks_exact(3) {
            let (a, b, c) = (p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]);
            let n = b.sub(a).cross(c.sub(a));
            assert!(n.dot(a.add(b).add(c)) > 0.0, "triangle folded inward");
            for (x, y) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *edges.entry((x.min(y), x.max(y))).or_default() += 1;
            }
        }
        assert!(edges.values().all(|&c| c == 2), "still watertight and manifold");
        for v in &p {
            assert!((v.length() - 1.0).abs() < 0.02, "vertex drifted off the sphere: {}", v.length());
        }
    }
}
