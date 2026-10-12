//! Reference meshes: load OBJ or glTF/GLB, compare a shape program against
//! them, and fit the program's parameters to them.
//!
//! This is how real-world data steers a compact program: export a body from
//! MakeHuman (OBJ) or take any rigged glTF character, then measure how far
//! each part of the program is from it, or let `fit` move the `$params`
//! (within their declared ranges) until the surfaces agree. Only parameter
//! values come back; no reference geometry is copied into the program.

use std::collections::HashMap;
use std::path::Path;

use avicreator_core::{Quat, Vec3};
use serde_json::Value;

use crate::compile::{compile, CompiledShape};
use crate::mesher::{mesh, MeshOptions};
use crate::program::ShapeProgram;
use crate::sdf::{triangle, Aabb};

/// Triangle soup in the program frame (metres, Y up, +Z forward).
pub struct Reference {
    pub triangles: Vec<[Vec3; 3]>,
    /// glTF meshes left out because they sit far outside the figure
    /// (hidden helpers, broken exports).
    pub dropped: Vec<String>,
    /// Placement applied to the source coordinates: `p -> rot * (p * scale) + shift`.
    pub scale: f32,
    pub rot: Quat,
    pub shift: Vec3,
}

impl Reference {
    /// Places a point given in the source file's coordinates like the mesh.
    pub fn place(&self, p: Vec3) -> Vec3 {
        self.rot.rotate_vec3(p.scale(self.scale)).add(self.shift)
    }
}

/// Anatomical landmarks in the reference's source coordinates:
/// `{"bones": {"mHipLeft": [x, y, z], ...}, "surface": {"Apex of nose": [x, y, z]}}`.
#[derive(Debug, Clone, Default)]
pub struct Landmarks {
    pub bones: Vec<(String, Vec3)>,
    pub surface: Vec<(String, Vec3)>,
}

impl Landmarks {
    pub fn load(path: &Path, reference: &Reference) -> Result<Landmarks, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))?;
        let read = |key: &str| -> Vec<(String, Vec3)> {
            v[key]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, p)| {
                    let a = p.as_array()?;
                    let c = |i: usize| a.get(i).and_then(Value::as_f64).map(|x| x as f32);
                    Some((k.clone(), reference.place(Vec3::new(c(0)?, c(1)?, c(2)?))))
                })
                .collect()
        };
        Ok(Landmarks { bones: read("bones"), surface: read("surface") })
    }

    /// Mean landmark error (metres): bone heads vs joint centres, and the
    /// program's surface distance at surface landmarks.
    pub fn error(&self, shape: &CompiledShape) -> f32 {
        let mut total = 0.0;
        let mut n = 0;
        for (name, p) in &self.bones {
            if let Some(b) = shape.bone_index(name) {
                total += shape.bones[b].head.sub(*p).length();
                n += 1;
            }
        }
        for (_, p) in &self.surface {
            total += shape.distance(*p).abs();
            n += 1;
        }
        if n == 0 { 0.0 } else { total / n as f32 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub scale: f32,
    pub rotate_y_degrees: f32,
    /// Centre on X/Z and stand the lowest point on Y = 0.
    pub ground: bool,
    /// Scale uniformly to this height after loading (compare proportions).
    pub height: Option<f32>,
}

impl Default for Placement {
    fn default() -> Self {
        Self { scale: 1.0, rotate_y_degrees: 0.0, ground: false, height: None }
    }
}

impl Reference {
    pub fn load(path: &Path, placement: Placement) -> Result<Reference, String> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        let mut dropped = Vec::new();
        let mut triangles = match ext.as_str() {
            "obj" => load_obj(path)?,
            "glb" | "gltf" => {
                let meshes = load_gltf(path)?;
                let (kept, out) = drop_outlier_meshes(meshes);
                dropped = out;
                kept
            }
            _ => return Err(format!("{}: expected .obj, .gltf or .glb", path.display())),
        };
        if triangles.is_empty() {
            return Err(format!("{}: no triangles found", path.display()));
        }
        let rot = Quat::from_axis_angle(Vec3::Y, placement.rotate_y_degrees.to_radians());
        let mut scale = placement.scale;
        if let Some(h) = placement.height {
            let current = bounds(&triangles).size().y * scale;
            if current > 0.0 {
                scale *= h / current;
            }
        }
        for t in &mut triangles {
            for v in t.iter_mut() {
                *v = rot.rotate_vec3(v.scale(scale));
            }
        }
        let mut shift = Vec3::ZERO;
        if placement.ground {
            let b = bounds(&triangles);
            shift = Vec3::new(-(b.min.x + b.max.x) * 0.5, -b.min.y, -(b.min.z + b.max.z) * 0.5);
            for t in &mut triangles {
                for v in t.iter_mut() {
                    *v = v.add(shift);
                }
            }
        }
        Ok(Reference { triangles, dropped, scale, rot, shift })
    }

    pub fn bounds(&self) -> Aabb {
        bounds(&self.triangles)
    }
}

/// Drops meshes whose centre lies well outside the robust (2nd to 98th
/// percentile) extent of all vertices.
fn drop_outlier_meshes(meshes: Vec<(String, Vec<[Vec3; 3]>)>) -> (Vec<[Vec3; 3]>, Vec<String>) {
    let mut axes: [Vec<f32>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (_, tris) in &meshes {
        for t in tris.iter().step_by(7) {
            for (a, c) in t[0].to_array().into_iter().enumerate() {
                axes[a].push(c);
            }
        }
    }
    let robust: Vec<(f32, f32)> = axes
        .iter_mut()
        .map(|v| {
            if v.is_empty() {
                return (f32::NEG_INFINITY, f32::INFINITY);
            }
            v.sort_by(f32::total_cmp);
            let lo = v[v.len() * 2 / 100];
            let hi = v[(v.len() * 98 / 100).min(v.len() - 1)];
            let pad = (hi - lo) * 0.25;
            (lo - pad, hi + pad)
        })
        .collect();
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for (name, tris) in meshes {
        let c = bounds(&tris).center().to_array();
        if (0..3).all(|a| c[a] >= robust[a].0 && c[a] <= robust[a].1) {
            kept.extend(tris);
        } else {
            dropped.push(name);
        }
    }
    (kept, dropped)
}

fn bounds(tris: &[[Vec3; 3]]) -> Aabb {
    tris.iter().flatten().fold(Aabb::EMPTY, |b, &p| b.union(Aabb::from_point(p, 0.0)))
}

fn load_obj(path: &Path) -> Result<Vec<[Vec3; 3]>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let mut verts = Vec::new();
    let mut tris = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let c: Vec<f32> = it.take(3).filter_map(|s| s.parse().ok()).collect();
                if c.len() == 3 {
                    verts.push(Vec3::new(c[0], c[1], c[2]));
                }
            }
            Some("f") => {
                let idx: Vec<usize> = it
                    .filter_map(|s| s.split('/').next()?.parse::<i64>().ok())
                    .map(|i| if i < 0 { (verts.len() as i64 + i) as usize } else { (i - 1) as usize })
                    .collect();
                for k in 1..idx.len().saturating_sub(1) {
                    if let (Some(&a), Some(&b), Some(&c)) = (verts.get(idx[0]), verts.get(idx[k]), verts.get(idx[k + 1])) {
                        tris.push([a, b, c]);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(tris)
}

type Mat4 = [f32; 16];

fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            out[c * 4 + r] = (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    out
}

fn transform(m: &Mat4, p: Vec3) -> Vec3 {
    Vec3::new(
        m[0] * p.x + m[4] * p.y + m[8] * p.z + m[12],
        m[1] * p.x + m[5] * p.y + m[9] * p.z + m[13],
        m[2] * p.x + m[6] * p.y + m[10] * p.z + m[14],
    )
}

const IDENTITY: Mat4 = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

fn node_matrix(node: &Value) -> Mat4 {
    if let Some(m) = node.get("matrix").and_then(Value::as_array) {
        let mut out = IDENTITY;
        for (i, v) in m.iter().take(16).enumerate() {
            out[i] = v.as_f64().unwrap_or(0.0) as f32;
        }
        return out;
    }
    let f = |key: &str, default: &[f32]| -> Vec<f32> {
        node.get(key)
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect())
            .unwrap_or_else(|| default.to_vec())
    };
    let t = f("translation", &[0.0, 0.0, 0.0]);
    let r = f("rotation", &[0.0, 0.0, 0.0, 1.0]);
    let s = f("scale", &[1.0, 1.0, 1.0]);
    let q = Quat::new(r[0], r[1], r[2], r[3]).normalize();
    let (cx, cy, cz) = (q.rotate_vec3(Vec3::X), q.rotate_vec3(Vec3::Y), q.rotate_vec3(Vec3::Z));
    [
        cx.x * s[0], cx.y * s[0], cx.z * s[0], 0.0,
        cy.x * s[1], cy.y * s[1], cy.z * s[1], 0.0,
        cz.x * s[2], cz.y * s[2], cz.z * s[2], 0.0,
        t[0], t[1], t[2], 1.0,
    ]
}

struct Gltf {
    doc: Value,
    buffers: Vec<Vec<u8>>,
}

impl Gltf {
    fn accessor_floats(&self, index: usize) -> Result<Vec<f32>, String> {
        let a = &self.doc["accessors"][index];
        let count = a["count"].as_u64().unwrap_or(0) as usize;
        let comps = match a["type"].as_str().unwrap_or("") {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            "MAT4" => 16,
            t => return Err(format!("unsupported accessor type {}", t)),
        };
        let Some(view_index) = a["bufferView"].as_u64() else { return Ok(vec![0.0; count * comps]) };
        let view = &self.doc["bufferViews"][view_index as usize];
        let buffer = &self.buffers[view["buffer"].as_u64().unwrap_or(0) as usize];
        let base = view["byteOffset"].as_u64().unwrap_or(0) as usize + a["byteOffset"].as_u64().unwrap_or(0) as usize;
        let ctype = a["componentType"].as_u64().unwrap_or(0);
        let size = match ctype {
            5126 | 5125 => 4,
            5123 | 5122 => 2,
            5121 | 5120 => 1,
            _ => return Err(format!("unsupported component type {}", ctype)),
        };
        let stride = view["byteStride"].as_u64().map(|s| s as usize).unwrap_or(size * comps);
        let normalized = a["normalized"].as_bool().unwrap_or(false);
        let mut out = Vec::with_capacity(count * comps);
        for i in 0..count {
            for c in 0..comps {
                let o = base + i * stride + c * size;
                let bytes = buffer.get(o..o + size).ok_or("accessor reads past its buffer")?;
                let v = match ctype {
                    5126 => f32::from_le_bytes(bytes.try_into().unwrap()),
                    5125 => u32::from_le_bytes(bytes.try_into().unwrap()) as f32,
                    5123 => {
                        let v = u16::from_le_bytes(bytes.try_into().unwrap()) as f32;
                        if normalized { v / 65535.0 } else { v }
                    }
                    5122 => {
                        let v = i16::from_le_bytes(bytes.try_into().unwrap()) as f32;
                        if normalized { (v / 32767.0).max(-1.0) } else { v }
                    }
                    5121 => {
                        let v = bytes[0] as f32;
                        if normalized { v / 255.0 } else { v }
                    }
                    _ => {
                        let v = bytes[0] as i8 as f32;
                        if normalized { (v / 127.0).max(-1.0) } else { v }
                    }
                };
                out.push(v);
            }
        }
        Ok(out)
    }
}

fn load_gltf(path: &Path) -> Result<Vec<(String, Vec<[Vec3; 3]>)>, String> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let (doc, glb_bin): (Value, Option<Vec<u8>>) = if data.starts_with(b"glTF") {
        let json_len = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
        let doc = serde_json::from_slice(&data[20..20 + json_len]).map_err(|e| e.to_string())?;
        let rest = 20 + json_len;
        let bin = (data.len() >= rest + 8).then(|| {
            let len = u32::from_le_bytes(data[rest..rest + 4].try_into().unwrap()) as usize;
            data[rest + 8..(rest + 8 + len).min(data.len())].to_vec()
        });
        (doc, bin)
    } else {
        (serde_json::from_slice(&data).map_err(|e| e.to_string())?, None)
    };
    if doc["extensionsRequired"].as_array().is_some_and(|e| e.iter().any(|x| x.as_str().is_some_and(|s| s.contains("draco") || s.contains("meshopt")))) {
        return Err("compressed glTF (Draco/meshopt) is not supported; re-export uncompressed".into());
    }
    let mut buffers = Vec::new();
    for (i, b) in doc["buffers"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        match b["uri"].as_str() {
            None if i == 0 => buffers.push(glb_bin.clone().ok_or("GLB without a BIN chunk")?),
            None => buffers.push(Vec::new()),
            Some(uri) if uri.starts_with("data:") => {
                return Err("embedded data-URI buffers are not supported; use .glb or external .bin".into())
            }
            Some(uri) => {
                let file = path.parent().unwrap_or(Path::new(".")).join(uri.replace("%20", " "));
                buffers.push(std::fs::read(&file).map_err(|e| format!("{}: {}", file.display(), e))?);
            }
        }
    }
    let g = Gltf { doc, buffers };

    // World matrices of every node reachable from the default scene.
    let nodes = g.doc["nodes"].as_array().cloned().unwrap_or_default();
    let mut world: HashMap<usize, Mat4> = HashMap::new();
    let scene = g.doc["scene"].as_u64().unwrap_or(0) as usize;
    let mut stack: Vec<(usize, Mat4)> = g.doc["scenes"][scene]["nodes"]
        .as_array()
        .map(|a| a.iter().filter_map(|n| n.as_u64()).map(|n| (n as usize, IDENTITY)).collect())
        .unwrap_or_else(|| (0..nodes.len()).map(|n| (n, IDENTITY)).collect());
    while let Some((n, parent)) = stack.pop() {
        let m = mat_mul(&parent, &node_matrix(&nodes[n]));
        world.insert(n, m);
        for c in nodes[n]["children"].as_array().into_iter().flatten().filter_map(Value::as_u64) {
            stack.push((c as usize, m));
        }
    }

    let mut meshes = Vec::new();
    for (n, node) in nodes.iter().enumerate() {
        let (Some(mesh_index), Some(node_world)) = (node["mesh"].as_u64(), world.get(&n)) else { continue };
        let mut tris = Vec::new();
        // Skinned meshes are posed exactly as a renderer would at rest:
        // v' = sum_k w_k * (world(joint_k) * inverseBind_k) * v.
        let joint_mats: Option<Vec<Mat4>> = match node["skin"].as_u64() {
            Some(s) => {
                let skin = &g.doc["skins"][s as usize];
                let joints: Vec<usize> = skin["joints"].as_array().into_iter().flatten().filter_map(Value::as_u64).map(|j| j as usize).collect();
                let ibms = match skin["inverseBindMatrices"].as_u64() {
                    Some(a) => g.accessor_floats(a as usize)?,
                    None => IDENTITY.repeat(joints.len()),
                };
                Some(
                    joints
                        .iter()
                        .enumerate()
                        .map(|(k, j)| {
                            let mut ibm = IDENTITY;
                            ibm.copy_from_slice(&ibms[k * 16..k * 16 + 16]);
                            mat_mul(world.get(j).unwrap_or(&IDENTITY), &ibm)
                        })
                        .collect(),
                )
            }
            None => None,
        };
        for prim in g.doc["meshes"][mesh_index as usize]["primitives"].as_array().into_iter().flatten() {
            if prim["mode"].as_u64().unwrap_or(4) != 4 {
                continue;
            }
            let attrs = &prim["attributes"];
            let Some(pos) = attrs["POSITION"].as_u64() else { continue };
            let p = g.accessor_floats(pos as usize)?;
            let skinning = match (&joint_mats, attrs["JOINTS_0"].as_u64(), attrs["WEIGHTS_0"].as_u64()) {
                (Some(mats), Some(j), Some(w)) => Some((mats, g.accessor_floats(j as usize)?, g.accessor_floats(w as usize)?)),
                _ => None,
            };
            let verts: Vec<Vec3> = p
                .chunks_exact(3)
                .enumerate()
                .map(|(i, c)| {
                    let v = Vec3::new(c[0], c[1], c[2]);
                    match &skinning {
                        Some((mats, joints, weights)) => {
                            let mut acc = Vec3::ZERO;
                            let mut total = 0.0;
                            for k in 0..4 {
                                let w = weights.get(i * 4 + k).copied().unwrap_or(0.0);
                                let j = joints.get(i * 4 + k).copied().unwrap_or(0.0) as usize;
                                if w > 0.0 {
                                    if let Some(m) = mats.get(j) {
                                        acc = acc.add(transform(m, v).scale(w));
                                        total += w;
                                    }
                                }
                            }
                            if total > 0.0 { acc.scale(1.0 / total) } else { transform(node_world, v) }
                        }
                        None => transform(node_world, v),
                    }
                })
                .collect();
            let idx: Vec<usize> = match prim["indices"].as_u64() {
                Some(i) => g.accessor_floats(i as usize)?.into_iter().map(|v| v as usize).collect(),
                None => (0..verts.len()).collect(),
            };
            for t in idx.chunks_exact(3) {
                if let (Some(&a), Some(&b), Some(&c)) = (verts.get(t[0]), verts.get(t[1]), verts.get(t[2])) {
                    tris.push([a, b, c]);
                }
            }
        }
        let name = node["name"].as_str().map(str::to_string).unwrap_or_else(|| format!("node {}", n));
        meshes.push((name, tris));
    }
    Ok(meshes)
}

/// Deterministic xorshift for reproducible sampling.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Area-weighted random points on a triangle soup.
pub fn sample_surface(tris: &[[Vec3; 3]], count: usize, seed: u64) -> Vec<Vec3> {
    let mut cumulative = Vec::with_capacity(tris.len());
    let mut total = 0.0f64;
    for t in tris {
        total += t[1].sub(t[0]).cross(t[2].sub(t[0])).length() as f64 * 0.5;
        cumulative.push(total);
    }
    if total <= 0.0 {
        return Vec::new();
    }
    let mut rng = Rng(seed.max(1));
    (0..count)
        .map(|_| {
            let target = rng.next() as f64 * total;
            let i = cumulative.partition_point(|&c| c < target).min(tris.len() - 1);
            let (mut u, mut v) = (rng.next(), rng.next());
            if u + v > 1.0 {
                u = 1.0 - u;
                v = 1.0 - v;
            }
            let t = &tris[i];
            t[0].add(t[1].sub(t[0]).scale(u)).add(t[2].sub(t[0]).scale(v))
        })
        .collect()
}

/// Uniform grid over triangles for nearest-surface queries.
pub struct TriangleGrid<'a> {
    tris: &'a [[Vec3; 3]],
    origin: Vec3,
    cell: f32,
    dims: [usize; 3],
    cells: HashMap<[usize; 3], Vec<u32>>,
}

impl<'a> TriangleGrid<'a> {
    pub fn new(tris: &'a [[Vec3; 3]]) -> Self {
        let b = bounds(tris);
        let size = b.size();
        let volume = (size.x * size.y * size.z).max(1e-9);
        let cell = (volume / tris.len().max(1) as f32).cbrt().max(1e-4) * 2.0;
        let dims = [
            (size.x / cell) as usize + 1,
            (size.y / cell) as usize + 1,
            (size.z / cell) as usize + 1,
        ];
        let mut grid = TriangleGrid { tris, origin: b.min, cell, dims, cells: HashMap::new() };
        for (i, t) in tris.iter().enumerate() {
            let tb = t.iter().fold(Aabb::EMPTY, |a, &p| a.union(Aabb::from_point(p, 0.0)));
            let (lo, hi) = (grid.cell_of(tb.min), grid.cell_of(tb.max));
            for z in lo[2]..=hi[2] {
                for y in lo[1]..=hi[1] {
                    for x in lo[0]..=hi[0] {
                        grid.cells.entry([x, y, z]).or_default().push(i as u32);
                    }
                }
            }
        }
        grid
    }

    fn cell_of(&self, p: Vec3) -> [usize; 3] {
        let r = p.sub(self.origin).scale(1.0 / self.cell);
        [
            (r.x.max(0.0) as usize).min(self.dims[0] - 1),
            (r.y.max(0.0) as usize).min(self.dims[1] - 1),
            (r.z.max(0.0) as usize).min(self.dims[2] - 1),
        ]
    }

    /// Unsigned distance from `p` to the nearest triangle.
    pub fn distance(&self, p: Vec3) -> f32 {
        let c = self.cell_of(p);
        let mut best = f32::INFINITY;
        let max_ring = self.dims[0].max(self.dims[1]).max(self.dims[2]);
        for ring in 0..=max_ring {
            let r = ring as isize;
            for z in -r..=r {
                for y in -r..=r {
                    for x in -r..=r {
                        if x.abs().max(y.abs()).max(z.abs()) != r {
                            continue;
                        }
                        let k = [c[0] as isize + x, c[1] as isize + y, c[2] as isize + z];
                        if k.iter().zip(&self.dims).any(|(&v, &d)| v < 0 || v >= d as isize) {
                            continue;
                        }
                        if let Some(list) = self.cells.get(&[k[0] as usize, k[1] as usize, k[2] as usize]) {
                            for &i in list {
                                let t = &self.tris[i as usize];
                                best = best.min(triangle(p, t[0], t[1], t[2]));
                            }
                        }
                    }
                }
            }
            // Everything outside this ring is at least `ring * cell` away.
            if best <= ring as f32 * self.cell {
                break;
            }
        }
        best
    }
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub mean_abs: f32,
    pub rms: f32,
    pub p95: f32,
    pub max: f32,
}

fn stats(values: &mut [f32]) -> Stats {
    if values.is_empty() {
        return Stats::default();
    }
    let n = values.len() as f32;
    let mean_abs = values.iter().map(|v| v.abs()).sum::<f32>() / n;
    let rms = (values.iter().map(|v| v * v).sum::<f32>() / n).sqrt();
    values.sort_by(|a, b| a.abs().total_cmp(&b.abs()));
    Stats { mean_abs, rms, p95: values[((n * 0.95) as usize).min(values.len() - 1)].abs(), max: values.last().unwrap().abs() }
}

#[derive(Debug, Clone, Default)]
pub struct PartReport {
    pub name: String,
    /// Mean signed distance of nearby reference surface from this part:
    /// positive = reference sticks out (part too small), negative = part too big.
    pub reference_offset: f32,
    pub reference_samples: usize,
    /// Mean distance from this part's surface to the reference.
    pub surface_gap: f32,
    pub surface_samples: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Reference surface measured against the program's distance field.
    pub reference_to_shape: Stats,
    /// Program surface measured against the reference triangles.
    pub shape_to_reference: Stats,
    pub parts: Vec<PartReport>,
}

impl Report {
    /// Symmetric mean surface distance (the fitting objective).
    pub fn score(&self) -> f32 {
        self.reference_to_shape.mean_abs + self.shape_to_reference.mean_abs
    }
}

fn owner(shape: &CompiledShape, p: Vec3) -> usize {
    (0..shape.parts.len())
        .filter(|&i| shape.parts[i].op == crate::program::Op::Union)
        .min_by(|&a, &b| shape.part_distance(a, p).total_cmp(&shape.part_distance(b, p)))
        .unwrap_or(0)
}

/// Compares a compiled shape with reference samples (see [`Report`]).
pub fn compare(shape: &CompiledShape, grid: &TriangleGrid, reference_samples: &[Vec3], resolution: u32, detail: bool) -> Report {
    let mut parts: Vec<PartReport> = shape.parts.iter().map(|p| PartReport { name: p.name.clone(), ..Default::default() }).collect();

    let mut ref_d: Vec<f32> = reference_samples.iter().map(|&p| shape.distance(p)).collect();
    if detail {
        for (&p, &d) in reference_samples.iter().zip(&ref_d) {
            let o = owner(shape, p);
            parts[o].reference_offset += d;
            parts[o].reference_samples += 1;
        }
    }

    let mut opts = MeshOptions::new(resolution);
    opts.skin = false;
    let m = mesh(shape, opts).mesh;
    let ours: Vec<[Vec3; 3]> = m
        .indices
        .chunks_exact(3)
        .map(|t| [m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]])
        .collect();
    let our_samples = sample_surface(&ours, reference_samples.len(), 0x5eed);
    let mut our_d: Vec<f32> = our_samples.iter().map(|&p| grid.distance(p)).collect();
    if detail {
        for (&p, &d) in our_samples.iter().zip(&our_d) {
            let o = owner(shape, p);
            parts[o].surface_gap += d;
            parts[o].surface_samples += 1;
        }
        for part in &mut parts {
            part.reference_offset /= part.reference_samples.max(1) as f32;
            part.surface_gap /= part.surface_samples.max(1) as f32;
        }
    }
    Report { reference_to_shape: stats(&mut ref_d), shape_to_reference: stats(&mut our_d), parts }
}

#[derive(Debug, Clone)]
pub struct FitResult {
    pub params: Vec<(String, f32, f32)>,
    pub before: f32,
    pub after: f32,
    pub evaluations: usize,
}

/// Coordinate descent on the chosen params within their slider ranges,
/// minimising the symmetric mean surface distance to the reference.
#[allow(clippy::too_many_arguments)]
pub fn fit(
    program: &mut ShapeProgram,
    names: &[String],
    grid: &TriangleGrid,
    samples: &[Vec3],
    landmarks: Option<&Landmarks>,
    resolution: u32,
    max_evaluations: usize,
    mut progress: impl FnMut(usize, f32),
) -> Result<FitResult, String> {
    let ranges: Vec<(String, f32, f32, f32)> = names
        .iter()
        .map(|n| {
            let p = program.params.get(n).ok_or_else(|| format!("unknown param '{}'", n))?;
            let (lo, hi) = p.range();
            Ok((n.clone(), p.value(), lo, hi))
        })
        .collect::<Result<_, String>>()?;
    let mut evaluations = 0;
    let mut score = |prog: &ShapeProgram| -> f32 {
        evaluations += 1;
        match compile(prog) {
            Ok(shape) => {
                compare(&shape, grid, samples, resolution, false).score() + landmarks.map_or(0.0, |l| l.error(&shape))
            }
            Err(_) => f32::INFINITY,
        }
    };
    let start = score(program);
    let mut best = start;
    progress(1, best);
    let mut steps: Vec<f32> = ranges.iter().map(|(_, _, lo, hi)| (hi - lo) * 0.2).collect();
    let mut evals = 1;
    'outer: for _round in 0..6 {
        let mut improved = true;
        while improved {
            improved = false;
            for (i, (name, _, lo, hi)) in ranges.iter().enumerate() {
                for dir in [1.0f32, -1.0] {
                    if evals >= max_evaluations {
                        break 'outer;
                    }
                    let current = program.params[name].value();
                    let candidate = (current + dir * steps[i]).clamp(*lo, *hi);
                    if (candidate - current).abs() < 1e-6 {
                        continue;
                    }
                    program.set_param(name, candidate).map_err(|e| e.to_string())?;
                    let s = score(program);
                    evals += 1;
                    if s < best {
                        best = s;
                        improved = true;
                        progress(evals, best);
                        break;
                    }
                    program.set_param(name, current).map_err(|e| e.to_string())?;
                }
            }
        }
        for s in &mut steps {
            *s *= 0.5;
        }
    }
    let params = ranges.iter().map(|(n, before, _, _)| (n.clone(), *before, program.params[n].value())).collect();
    Ok(FitResult { params, before: start, after: best, evaluations: evals })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube_obj() -> String {
        // Unit cube centred at (0, 1, 0), as quads.
        "v -0.5 0.5 -0.5\nv 0.5 0.5 -0.5\nv 0.5 1.5 -0.5\nv -0.5 1.5 -0.5\n\
         v -0.5 0.5 0.5\nv 0.5 0.5 0.5\nv 0.5 1.5 0.5\nv -0.5 1.5 0.5\n\
         f 1 2 3 4\nf 5 8 7 6\nf 1 5 6 2\nf 2 6 7 3\nf 3 7 8 4\nf 4 8 5 1\n"
            .to_string()
    }

    #[test]
    fn obj_loads_quads_and_grid_matches_brute_force() {
        let dir = std::env::temp_dir().join(format!("avishape-ref-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cube.obj");
        std::fs::write(&path, cube_obj()).unwrap();
        let r = Reference::load(&path, Placement { ground: true, ..Default::default() }).unwrap();
        assert_eq!(r.triangles.len(), 12);
        assert!((r.bounds().min.y).abs() < 1e-6, "grounded");

        let grid = TriangleGrid::new(&r.triangles);
        for p in [Vec3::new(0.0, 0.5, 0.0), Vec3::new(2.0, 3.0, -1.0), Vec3::new(0.1, 0.2, 0.45)] {
            let brute = r.triangles.iter().map(|t| triangle(p, t[0], t[1], t[2])).fold(f32::INFINITY, f32::min);
            assert!((grid.distance(p) - brute).abs() < 1e-5);
        }
        let samples = sample_surface(&r.triangles, 500, 7);
        assert!(samples.iter().all(|&p| grid.distance(p) < 1e-4));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn fit_recovers_a_known_radius() {
        // Reference: a sphere of radius 0.3 meshed by the engine itself.
        let reference = compile(&ShapeProgram::from_json(r#"{"parts":[{"kind":"blob","name":"b","from":[0,1,0],"radii":0.3}]}"#).unwrap()).unwrap();
        let m = mesh(&reference, MeshOptions::new(48)).mesh;
        let tris: Vec<[Vec3; 3]> = m.indices.chunks_exact(3).map(|t| [m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]]).collect();
        let grid = TriangleGrid::new(&tris);
        let samples = sample_surface(&tris, 800, 3);

        let mut program = ShapeProgram::from_json(
            r#"{"params":{"r":{"value":0.18,"min":0.1,"max":0.5}},"parts":[{"kind":"blob","name":"b","from":[0,1,0],"radii":"$r"}]}"#,
        )
        .unwrap();
        let result = fit(&mut program, &["r".to_string()], &grid, &samples, None, 32, 60, |_, _| {}).unwrap();
        let r = program.params["r"].value();
        assert!((r - 0.3).abs() < 0.01, "fitted radius {}", r);
        assert!(result.after < result.before * 0.2);
    }
}
