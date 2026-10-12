//! Binary glTF 2.0 (`.glb`) export of a skinned mesh and its skeleton.
//!
//! Joint 0 is an added `root` at the origin so every skin has a common root;
//! bone `b` is joint `b + 1`. Each joint's local +Y runs along its bone and
//! +X is its bend axis; rotation limits ride along in node `extras`.

use avicreator_core::{Mesh, Quat, Vec3};
use serde_json::{json, Value};

use crate::compile::CompiledShape;

const FLOAT: u32 = 5126;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;

#[derive(Debug, Clone, Copy)]
pub struct GlbOptions {
    /// Adds a looping "flex" clip that bends every chain within its limits.
    pub animation: bool,
    pub clip_seconds: f32,
}

impl Default for GlbOptions {
    fn default() -> Self {
        Self { animation: true, clip_seconds: 2.0 }
    }
}

#[derive(Default)]
struct Builder {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Builder {
    fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while self.bin.len() % 4 != 0 {
            self.bin.push(0);
        }
        let mut v = json!({ "buffer": 0, "byteOffset": self.bin.len(), "byteLength": bytes.len() });
        if let Some(t) = target {
            v["target"] = json!(t);
        }
        self.bin.extend_from_slice(bytes);
        self.views.push(v);
        self.views.len() - 1
    }

    fn accessor(&mut self, bytes: &[u8], target: Option<u32>, component: u32, count: usize, ty: &str, minmax: Option<(Vec<f32>, Vec<f32>)>) -> usize {
        let view = self.view(bytes, target);
        let mut a = json!({ "bufferView": view, "componentType": component, "count": count, "type": ty });
        if let Some((min, max)) = minmax {
            a["min"] = json!(min);
            a["max"] = json!(max);
        }
        self.accessors.push(a);
        self.accessors.len() - 1
    }
}

fn f32_bytes(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values.into_iter().flat_map(f32::to_le_bytes).collect()
}

fn vec3_minmax(points: &[Vec3]) -> (Vec<f32>, Vec<f32>) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in points {
        for (i, c) in p.to_array().into_iter().enumerate() {
            min[i] = min[i].min(c);
            max[i] = max[i].max(c);
        }
    }
    (min.to_vec(), max.to_vec())
}

/// Column-major inverse of the rigid transform `T(head) * R(rest)`.
fn inverse_bind(head: Vec3, rest: Quat) -> [f32; 16] {
    let inv = rest.conjugate();
    let (c0, c1, c2) = (inv.rotate_vec3(Vec3::X), inv.rotate_vec3(Vec3::Y), inv.rotate_vec3(Vec3::Z));
    let t = inv.rotate_vec3(head).scale(-1.0);
    [c0.x, c0.y, c0.z, 0.0, c1.x, c1.y, c1.z, 0.0, c2.x, c2.y, c2.z, 0.0, t.x, t.y, t.z, 1.0]
}

/// Bend angle (radians) for keyframe phase `s` in -1..1, kept inside limits.
fn flex_angle(limits: [f32; 2], s: f32) -> f32 {
    let centre = 0.0f32.clamp(limits[0], limits[1]);
    let room = if s >= 0.0 { limits[1] - centre } else { centre - limits[0] };
    (centre + 0.35 * room * s).to_radians()
}

pub fn write_glb(shape: &CompiledShape, mesh: &Mesh, opts: GlbOptions) -> Vec<u8> {
    let mut b = Builder::default();
    let skinned = !shape.bones.is_empty();
    let count = mesh.positions.len();

    let mut attributes = json!({
        "POSITION": b.accessor(&f32_bytes(mesh.positions.iter().flat_map(|p| p.to_array())), Some(ARRAY_BUFFER), FLOAT, count, "VEC3", Some(vec3_minmax(&mesh.positions))),
        "NORMAL": b.accessor(&f32_bytes(mesh.normals.iter().flat_map(|p| p.to_array())), Some(ARRAY_BUFFER), FLOAT, count, "VEC3", None),
    });
    if skinned {
        let joints: Vec<u8> = mesh
            .bone_indices
            .iter()
            .zip(&mesh.bone_weights)
            .flat_map(|(idx, w)| {
                // Shift past the root joint; unused slots point at the root with zero weight.
                (0..4).map(move |s| if w[s] > 0.0 { idx[s] as u16 + 1 } else { 0 })
            })
            .flat_map(u16::to_le_bytes)
            .collect();
        attributes["JOINTS_0"] = json!(b.accessor(&joints, Some(ARRAY_BUFFER), UNSIGNED_SHORT, count, "VEC4", None));
        let weights = f32_bytes(mesh.bone_weights.iter().flatten().copied());
        attributes["WEIGHTS_0"] = json!(b.accessor(&weights, Some(ARRAY_BUFFER), FLOAT, count, "VEC4", None));
    }
    let index_bytes: Vec<u8> = mesh.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let indices = b.accessor(&index_bytes, Some(ELEMENT_ARRAY_BUFFER), UNSIGNED_INT, mesh.indices.len(), "SCALAR", None);

    let mut mesh_node = json!({ "name": shape.name, "mesh": 0 });
    let mut nodes = vec![];
    let mut scene_nodes = vec![0];
    let mut skins = vec![];
    let mut animations = vec![];

    if skinned {
        mesh_node["skin"] = json!(0);
        let root_node = 1usize;
        let joint_node = |bone: usize| bone + 2;
        let mut root = json!({ "name": "root", "children": [] });
        let mut joint_nodes = Vec::new();
        let mut local_rest = Vec::new();
        for (i, bone) in shape.bones.iter().enumerate() {
            let (p_head, p_rot) = match bone.parent {
                Some(p) => (shape.bones[p].head, shape.bones[p].rest),
                None => (Vec3::ZERO, Quat::IDENTITY),
            };
            let inv_parent = p_rot.conjugate();
            let translation = inv_parent.rotate_vec3(bone.head.sub(p_head));
            let rotation = inv_parent.mul(bone.rest).normalize();
            local_rest.push(rotation);
            let children: Vec<usize> = shape
                .bones
                .iter()
                .enumerate()
                .filter(|(_, c)| c.parent == Some(i))
                .map(|(c, _)| joint_node(c))
                .collect();
            let mut node = json!({
                "name": bone.name,
                "translation": translation.to_array(),
                "rotation": rotation.to_array(),
                "extras": {
                    "part": shape.parts[bone.part].name,
                    "length": bone.tail.sub(bone.head).length(),
                    "limits": { "bend": bone.limits.bend, "side": bone.limits.side, "twist": bone.limits.twist },
                },
            });
            if !children.is_empty() {
                node["children"] = json!(children);
            }
            if bone.parent.is_none() {
                root["children"].as_array_mut().unwrap().push(json!(joint_node(i)));
            }
            joint_nodes.push(node);
        }
        nodes.push(mesh_node);
        nodes.push(root);
        nodes.extend(joint_nodes);
        scene_nodes.push(root_node);

        let mut ibm = f32_bytes(inverse_bind(Vec3::ZERO, Quat::IDENTITY));
        for bone in &shape.bones {
            ibm.extend(f32_bytes(inverse_bind(bone.head, bone.rest)));
        }
        let ibm_accessor = b.accessor(&ibm, None, FLOAT, shape.bones.len() + 1, "MAT4", None);
        let joints: Vec<usize> = std::iter::once(root_node).chain((0..shape.bones.len()).map(joint_node)).collect();
        skins.push(json!({ "name": format!("{}_skin", shape.name), "joints": joints, "skeleton": root_node, "inverseBindMatrices": ibm_accessor }));

        if opts.animation {
            let frames = 25;
            let times: Vec<f32> = (0..frames).map(|k| opts.clip_seconds * k as f32 / (frames - 1) as f32).collect();
            let time_accessor = b.accessor(&f32_bytes(times.iter().copied()), None, FLOAT, frames, "SCALAR", Some((vec![0.0], vec![opts.clip_seconds])));
            let mut samplers = Vec::new();
            let mut channels = Vec::new();
            for (i, bone) in shape.bones.iter().enumerate() {
                if bone.parent.is_none() || shape.parts[bone.part].kind != "chain" {
                    continue;
                }
                let depth = shape.bones[..i].iter().filter(|o| o.part == bone.part).count() as f32;
                let keys: Vec<f32> = times
                    .iter()
                    .flat_map(|&t| {
                        let s = (std::f32::consts::TAU * t / opts.clip_seconds - depth * 0.6).sin();
                        let bend = Quat::from_axis_angle(Vec3::X, flex_angle(bone.limits.bend, s));
                        local_rest[i].mul(bend).normalize().to_array()
                    })
                    .collect();
                let out = b.accessor(&f32_bytes(keys), None, FLOAT, frames, "VEC4", None);
                samplers.push(json!({ "input": time_accessor, "output": out, "interpolation": "LINEAR" }));
                channels.push(json!({ "sampler": samplers.len() - 1, "target": { "node": joint_node(i), "path": "rotation" } }));
            }
            if !channels.is_empty() {
                animations.push(json!({ "name": "flex", "samplers": samplers, "channels": channels }));
            }
        }
    } else {
        nodes.push(mesh_node);
    }

    let mut doc = json!({
        "asset": { "version": "2.0", "generator": "avicreator-shape" },
        "scene": 0,
        "scenes": [{ "name": shape.name, "nodes": scene_nodes, "extras": { "params": shape.params } }],
        "nodes": nodes,
        "meshes": [{ "name": shape.name, "primitives": [{ "attributes": attributes, "indices": indices, "material": 0 }] }],
        "materials": [{ "name": "clay", "pbrMetallicRoughness": { "baseColorFactor": [0.78, 0.72, 0.66, 1.0], "metallicFactor": 0.0, "roughnessFactor": 0.7 } }],
    });
    if !skins.is_empty() {
        doc["skins"] = json!(skins);
    }
    if !animations.is_empty() {
        doc["animations"] = json!(animations);
    }
    while b.bin.len() % 4 != 0 {
        b.bin.push(0);
    }
    doc["accessors"] = json!(b.accessors);
    doc["bufferViews"] = json!(b.views);
    doc["buffers"] = json!([{ "byteLength": b.bin.len() }]);

    let mut json_bytes = serde_json::to_vec(&doc).expect("glTF JSON serializes");
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12 + 8 + json_bytes.len() + 8 + b.bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(b.bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&b.bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flex_stays_within_limits() {
        for limits in [[-45.0, 45.0], [0.0, 120.0], [10.0, 30.0], [-5.0, 0.0]] {
            for s in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                let a = flex_angle(limits, s).to_degrees();
                assert!(a >= limits[0] - 1e-3 && a <= limits[1] + 1e-3, "{:?} {} -> {}", limits, s, a);
            }
        }
    }

    #[test]
    fn inverse_bind_undoes_the_rest_transform() {
        let rest = Quat::from_axis_angle(Vec3::new(1.0, 2.0, 0.5), 0.8);
        let head = Vec3::new(0.3, 1.0, -0.2);
        let m = inverse_bind(head, rest);
        let world = head.add(rest.rotate_vec3(Vec3::new(0.0, 0.5, 0.0)));
        let local = Vec3::new(
            m[0] * world.x + m[4] * world.y + m[8] * world.z + m[12],
            m[1] * world.x + m[5] * world.y + m[9] * world.z + m[13],
            m[2] * world.x + m[6] * world.y + m[10] * world.z + m[14],
        );
        assert!(local.sub(Vec3::new(0.0, 0.5, 0.0)).length() < 1e-5);
    }
}
