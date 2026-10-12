//! Second Life (Bento) skeleton support: joint names and parenting,
//! validation of programs that target it, and rigged Collada export for
//! the SL mesh uploader.
//!
//! Programs name their bones after SL joints (`mPelvis`, `mHipLeft`,
//! `mTail1`, …). Bones without an SL name are helpers (horn segments, ear
//! tips): they shape weights while modelling and fold into their nearest SL
//! ancestor on export. Joint *positions* come from the program, so custom
//! proportions (digitigrade legs, long muzzles, tails) upload with
//! "Include joint positions" enabled; joint orientations stay SL's fixed
//! avatar-aligned frames, which is what SL animations expect.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use avicreator_core::{Mesh, Vec3};

use crate::compile::CompiledShape;

/// `(joint, parent)` in hierarchy order: every parent precedes its children.
/// Covers the classic body and the Bento extensions: spine, tail, groin,
/// hind limbs, wings, fingers and the face rig (brows, lids, cheeks, nose,
/// lips, teeth, tongue, jaw, ears).
pub const JOINTS: &[(&str, Option<&str>)] = &[
    ("mPelvis", None),
    ("mSpine1", Some("mPelvis")),
    ("mSpine2", Some("mSpine1")),
    ("mTorso", Some("mSpine2")),
    ("mSpine3", Some("mTorso")),
    ("mSpine4", Some("mSpine3")),
    ("mChest", Some("mSpine4")),
    ("mNeck", Some("mChest")),
    ("mHead", Some("mNeck")),
    ("mSkull", Some("mHead")),
    ("mEyeRight", Some("mHead")),
    ("mEyeLeft", Some("mHead")),
    ("mFaceRoot", Some("mHead")),
    ("mFaceEyeAltRight", Some("mFaceRoot")),
    ("mFaceEyeAltLeft", Some("mFaceRoot")),
    ("mFaceForeheadLeft", Some("mFaceRoot")),
    ("mFaceForeheadRight", Some("mFaceRoot")),
    ("mFaceEyebrowOuterLeft", Some("mFaceRoot")),
    ("mFaceEyebrowCenterLeft", Some("mFaceRoot")),
    ("mFaceEyebrowInnerLeft", Some("mFaceRoot")),
    ("mFaceEyebrowOuterRight", Some("mFaceRoot")),
    ("mFaceEyebrowCenterRight", Some("mFaceRoot")),
    ("mFaceEyebrowInnerRight", Some("mFaceRoot")),
    ("mFaceEyeLidUpperLeft", Some("mFaceRoot")),
    ("mFaceEyeLidLowerLeft", Some("mFaceRoot")),
    ("mFaceEyeLidUpperRight", Some("mFaceRoot")),
    ("mFaceEyeLidLowerRight", Some("mFaceRoot")),
    ("mFaceEar1Left", Some("mFaceRoot")),
    ("mFaceEar2Left", Some("mFaceEar1Left")),
    ("mFaceEar1Right", Some("mFaceRoot")),
    ("mFaceEar2Right", Some("mFaceEar1Right")),
    ("mFaceNoseLeft", Some("mFaceRoot")),
    ("mFaceNoseCenter", Some("mFaceRoot")),
    ("mFaceNoseRight", Some("mFaceRoot")),
    ("mFaceCheekLowerLeft", Some("mFaceRoot")),
    ("mFaceCheekUpperLeft", Some("mFaceRoot")),
    ("mFaceCheekLowerRight", Some("mFaceRoot")),
    ("mFaceCheekUpperRight", Some("mFaceRoot")),
    ("mFaceJaw", Some("mFaceRoot")),
    ("mFaceChin", Some("mFaceJaw")),
    ("mFaceTeethLower", Some("mFaceJaw")),
    ("mFaceLipLowerLeft", Some("mFaceTeethLower")),
    ("mFaceLipLowerRight", Some("mFaceTeethLower")),
    ("mFaceLipLowerCenter", Some("mFaceTeethLower")),
    ("mFaceTongueBase", Some("mFaceTeethLower")),
    ("mFaceTongueTip", Some("mFaceTongueBase")),
    ("mFaceJawShaper", Some("mFaceRoot")),
    ("mFaceForeheadCenter", Some("mFaceRoot")),
    ("mFaceNoseBase", Some("mFaceRoot")),
    ("mFaceTeethUpper", Some("mFaceRoot")),
    ("mFaceLipUpperLeft", Some("mFaceTeethUpper")),
    ("mFaceLipUpperRight", Some("mFaceTeethUpper")),
    ("mFaceLipCornerLeft", Some("mFaceTeethUpper")),
    ("mFaceLipCornerRight", Some("mFaceTeethUpper")),
    ("mFaceLipUpperCenter", Some("mFaceTeethUpper")),
    ("mFaceEyecornerInnerLeft", Some("mFaceRoot")),
    ("mFaceEyecornerInnerRight", Some("mFaceRoot")),
    ("mFaceNoseBridge", Some("mFaceRoot")),
    ("mCollarLeft", Some("mChest")),
    ("mShoulderLeft", Some("mCollarLeft")),
    ("mElbowLeft", Some("mShoulderLeft")),
    ("mWristLeft", Some("mElbowLeft")),
    ("mHandMiddle1Left", Some("mWristLeft")),
    ("mHandMiddle2Left", Some("mHandMiddle1Left")),
    ("mHandMiddle3Left", Some("mHandMiddle2Left")),
    ("mHandIndex1Left", Some("mWristLeft")),
    ("mHandIndex2Left", Some("mHandIndex1Left")),
    ("mHandIndex3Left", Some("mHandIndex2Left")),
    ("mHandRing1Left", Some("mWristLeft")),
    ("mHandRing2Left", Some("mHandRing1Left")),
    ("mHandRing3Left", Some("mHandRing2Left")),
    ("mHandPinky1Left", Some("mWristLeft")),
    ("mHandPinky2Left", Some("mHandPinky1Left")),
    ("mHandPinky3Left", Some("mHandPinky2Left")),
    ("mHandThumb1Left", Some("mWristLeft")),
    ("mHandThumb2Left", Some("mHandThumb1Left")),
    ("mHandThumb3Left", Some("mHandThumb2Left")),
    ("mCollarRight", Some("mChest")),
    ("mShoulderRight", Some("mCollarRight")),
    ("mElbowRight", Some("mShoulderRight")),
    ("mWristRight", Some("mElbowRight")),
    ("mHandMiddle1Right", Some("mWristRight")),
    ("mHandMiddle2Right", Some("mHandMiddle1Right")),
    ("mHandMiddle3Right", Some("mHandMiddle2Right")),
    ("mHandIndex1Right", Some("mWristRight")),
    ("mHandIndex2Right", Some("mHandIndex1Right")),
    ("mHandIndex3Right", Some("mHandIndex2Right")),
    ("mHandRing1Right", Some("mWristRight")),
    ("mHandRing2Right", Some("mHandRing1Right")),
    ("mHandRing3Right", Some("mHandRing2Right")),
    ("mHandPinky1Right", Some("mWristRight")),
    ("mHandPinky2Right", Some("mHandPinky1Right")),
    ("mHandPinky3Right", Some("mHandPinky2Right")),
    ("mHandThumb1Right", Some("mWristRight")),
    ("mHandThumb2Right", Some("mHandThumb1Right")),
    ("mHandThumb3Right", Some("mHandThumb2Right")),
    ("mWingsRoot", Some("mChest")),
    ("mWing1Left", Some("mWingsRoot")),
    ("mWing2Left", Some("mWing1Left")),
    ("mWing3Left", Some("mWing2Left")),
    ("mWing4Left", Some("mWing3Left")),
    ("mWing4FanLeft", Some("mWing3Left")),
    ("mWing1Right", Some("mWingsRoot")),
    ("mWing2Right", Some("mWing1Right")),
    ("mWing3Right", Some("mWing2Right")),
    ("mWing4Right", Some("mWing3Right")),
    ("mWing4FanRight", Some("mWing3Right")),
    ("mHipRight", Some("mPelvis")),
    ("mKneeRight", Some("mHipRight")),
    ("mAnkleRight", Some("mKneeRight")),
    ("mFootRight", Some("mAnkleRight")),
    ("mToeRight", Some("mFootRight")),
    ("mHipLeft", Some("mPelvis")),
    ("mKneeLeft", Some("mHipLeft")),
    ("mAnkleLeft", Some("mKneeLeft")),
    ("mFootLeft", Some("mAnkleLeft")),
    ("mToeLeft", Some("mFootLeft")),
    ("mTail1", Some("mPelvis")),
    ("mTail2", Some("mTail1")),
    ("mTail3", Some("mTail2")),
    ("mTail4", Some("mTail3")),
    ("mTail5", Some("mTail4")),
    ("mTail6", Some("mTail5")),
    ("mGroin", Some("mPelvis")),
    ("mHindLimbsRoot", Some("mPelvis")),
    ("mHindLimb1Left", Some("mHindLimbsRoot")),
    ("mHindLimb2Left", Some("mHindLimb1Left")),
    ("mHindLimb3Left", Some("mHindLimb2Left")),
    ("mHindLimb4Left", Some("mHindLimb3Left")),
    ("mHindLimb1Right", Some("mHindLimbsRoot")),
    ("mHindLimb2Right", Some("mHindLimb1Right")),
    ("mHindLimb3Right", Some("mHindLimb2Right")),
    ("mHindLimb4Right", Some("mHindLimb3Right")),
];

fn parents() -> HashMap<&'static str, Option<&'static str>> {
    JOINTS.iter().copied().collect()
}

/// True if `ancestor` is a strict ancestor of `joint` in the SL hierarchy.
fn is_ancestor(table: &HashMap<&str, Option<&str>>, ancestor: &str, joint: &str) -> bool {
    let mut cur = table.get(joint).copied().flatten();
    while let Some(p) = cur {
        if p == ancestor {
            return true;
        }
        cur = table.get(p).copied().flatten();
    }
    false
}

/// Names shaped like SL joints (`m` + capital) must be real SL joints.
fn looks_like_sl(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next() == Some('m') && chars.next().is_some_and(|c| c.is_ascii_uppercase())
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// Nearest bone at or above `bone` whose name is an SL joint.
fn sl_target(shape: &CompiledShape, table: &HashMap<&str, Option<&str>>, bone: usize) -> Option<usize> {
    let mut cur = Some(bone);
    while let Some(b) = cur {
        if table.contains_key(shape.bones[b].name.as_str()) {
            return Some(b);
        }
        cur = shape.bones[b].parent;
    }
    None
}

/// Checks names and parenting against the SL hierarchy, with fixes in the messages.
pub fn validate(shape: &CompiledShape) -> Result<(), String> {
    let table = parents();
    for bone in &shape.bones {
        if looks_like_sl(&bone.name) && !table.contains_key(bone.name.as_str()) {
            let best = JOINTS.iter().map(|(n, _)| *n).min_by_key(|n| edit_distance(n, &bone.name)).unwrap_or("mPelvis");
            return Err(format!("'{}' is not a Second Life joint (did you mean {}?)", bone.name, best));
        }
    }
    if !shape.bones.iter().any(|b| b.name == "mPelvis") {
        return Err("Second Life skeletons are rooted at mPelvis; name the hip/pelvis bone \"mPelvis\"".into());
    }
    for bone in &shape.bones {
        let above = bone.parent.and_then(|p| sl_target(shape, &table, p));
        let is_sl = table.contains_key(bone.name.as_str());
        match (is_sl, above) {
            (true, None) if bone.name != "mPelvis" => {
                return Err(format!("{} must sit under mPelvis, but its part is not attached below any SL joint", bone.name));
            }
            (true, Some(a)) if !is_ancestor(&table, &shape.bones[a].name, &bone.name) => {
                return Err(format!(
                    "{} is attached below {}, but in Second Life {} is not an ancestor of {}; attach it under {}",
                    bone.name,
                    shape.bones[a].name,
                    shape.bones[a].name,
                    bone.name,
                    table[bone.name.as_str()].unwrap_or("nothing (it is the root)")
                ));
            }
            (false, None) => {
                return Err(format!(
                    "helper bone '{}' has no Second Life joint above it; attach its part below an SL bone",
                    bone.name
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Program frame (Y up, +Z forward, +X left) to SL's (Z up, +X forward, +Y left).
/// A cyclic axis permutation, so it is a proper rotation and keeps winding.
fn to_sl(v: Vec3) -> Vec3 {
    Vec3::new(v.z, v.x, v.y)
}

/// Joints written to the `.dae`: every SL-named bone plus the SL ancestors
/// needed to keep the hierarchy intact, in hierarchy order, with positions.
pub struct SlRig {
    pub joints: Vec<(&'static str, Option<usize>, Vec3)>,
    /// Program bone index -> exported joint index (helpers fold upward).
    pub bone_to_joint: Vec<usize>,
}

pub fn rig(shape: &CompiledShape) -> SlRig {
    let table = parents();
    let named: HashMap<&str, usize> =
        shape.bones.iter().enumerate().filter(|(_, b)| table.contains_key(b.name.as_str())).map(|(i, b)| (b.name.as_str(), i)).collect();
    let mut needed: HashSet<&str> = HashSet::new();
    for name in named.keys() {
        let mut cur = Some(*name);
        while let Some(n) = cur {
            if !needed.insert(n) {
                break;
            }
            cur = table.get(n).copied().flatten();
        }
    }
    let mut joints: Vec<(&'static str, Option<usize>, Vec3)> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for &(name, parent) in JOINTS {
        if !needed.contains(name) {
            continue;
        }
        let parent_index = parent.map(|p| index[p]);
        // Joints the program does not shape sit on their parent (zero offset),
        // which is how SL's own spine helper joints rest.
        let pos = match named.get(name) {
            Some(&b) => shape.bones[b].head,
            None => parent_index.map_or(Vec3::ZERO, |p| joints[p].2),
        };
        index.insert(name, joints.len());
        joints.push((name, parent_index, pos));
    }
    let bone_to_joint = (0..shape.bones.len())
        .map(|b| sl_target(shape, &table, b).map_or(0, |t| index[shape.bones[t].name.as_str()]))
        .collect();
    SlRig { joints, bone_to_joint }
}

fn id(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    if s.is_empty() { "shape".into() } else { s }
}

/// Rigged Collada 1.4.1 for the Second Life mesh uploader.
pub fn write_collada(shape: &CompiledShape, mesh: &Mesh) -> String {
    let rig = rig(shape);
    let name = id(&shape.name);
    let count = mesh.positions.len();
    let mut x = String::with_capacity(count * 120);
    let _ = writeln!(x, r#"<?xml version="1.0" encoding="utf-8"?>"#);
    let _ = writeln!(x, r#"<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">"#);
    let _ = writeln!(
        x,
        r#"  <asset><contributor><authoring_tool>avicreator-shape</authoring_tool></contributor><created>2000-01-01T00:00:00Z</created><modified>2000-01-01T00:00:00Z</modified><unit name="meter" meter="1"/><up_axis>Z_UP</up_axis></asset>"#
    );
    let _ = writeln!(
        x,
        r#"  <library_effects><effect id="clay-effect"><profile_COMMON><technique sid="common"><lambert><diffuse><color sid="diffuse">0.78 0.72 0.66 1</color></diffuse></lambert></technique></profile_COMMON></effect></library_effects>"#
    );
    let _ = writeln!(x, r##"  <library_materials><material id="clay-material" name="clay"><instance_effect url="#clay-effect"/></material></library_materials>"##);

    let floats = |values: &mut dyn Iterator<Item = f32>| values.map(|v| format!("{:.6}", v)).collect::<Vec<_>>().join(" ");
    let positions = floats(&mut mesh.positions.iter().flat_map(|p| to_sl(*p).to_array()));
    let normals = floats(&mut mesh.normals.iter().flat_map(|p| to_sl(*p).to_array()));
    let _ = writeln!(x, r#"  <library_geometries><geometry id="{n}-mesh" name="{n}"><mesh>"#, n = name);
    for (kind, data) in [("positions", &positions), ("normals", &normals)] {
        let _ = writeln!(
            x,
            r##"    <source id="{n}-{k}"><float_array id="{n}-{k}-array" count="{c3}">{d}</float_array><technique_common><accessor source="#{n}-{k}-array" count="{c}" stride="3"><param name="X" type="float"/><param name="Y" type="float"/><param name="Z" type="float"/></accessor></technique_common></source>"##,
            n = name,
            k = kind,
            c3 = count * 3,
            c = count,
            d = data
        );
    }
    let _ = writeln!(x, r##"    <vertices id="{n}-vertices"><input semantic="POSITION" source="#{n}-positions"/></vertices>"##, n = name);
    let p: Vec<String> = mesh.indices.iter().map(|i| format!("{} {}", i, i)).collect();
    let _ = writeln!(
        x,
        r##"    <triangles material="clay" count="{t}"><input semantic="VERTEX" source="#{n}-vertices" offset="0"/><input semantic="NORMAL" source="#{n}-normals" offset="1"/><p>{p}</p></triangles>"##,
        n = name,
        t = mesh.indices.len() / 3,
        p = p.join(" ")
    );
    let _ = writeln!(x, "  </mesh></geometry></library_geometries>");

    // Skin: per-vertex weights folded onto exported joints.
    let mut weights: Vec<f32> = Vec::new();
    let mut vcount: Vec<String> = Vec::with_capacity(count);
    let mut v: Vec<String> = Vec::new();
    for (idx, w) in mesh.bone_indices.iter().zip(&mesh.bone_weights) {
        let mut acc: Vec<(usize, f32)> = Vec::new();
        for s in 0..4 {
            if w[s] <= 0.0 || idx[s] as usize >= rig.bone_to_joint.len() {
                continue;
            }
            let j = rig.bone_to_joint[idx[s] as usize];
            match acc.iter_mut().find(|e| e.0 == j) {
                Some(e) => e.1 += w[s],
                None => acc.push((j, w[s])),
            }
        }
        acc.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        acc.truncate(4);
        let sum: f32 = acc.iter().map(|e| e.1).sum();
        if acc.is_empty() || sum <= 0.0 {
            acc = vec![(0, 1.0)];
        }
        let sum: f32 = acc.iter().map(|e| e.1).sum();
        vcount.push(acc.len().to_string());
        for (j, wt) in acc {
            v.push(format!("{} {}", j, weights.len()));
            weights.push(wt / sum);
        }
    }
    let joint_names: Vec<&str> = rig.joints.iter().map(|j| j.0).collect();
    let bind_poses: Vec<String> = rig
        .joints
        .iter()
        .map(|j| {
            let t = to_sl(j.2);
            format!("1 0 0 {:.6} 0 1 0 {:.6} 0 0 1 {:.6} 0 0 0 1", -t.x, -t.y, -t.z)
        })
        .collect();
    let jc = joint_names.len();
    let _ = writeln!(x, r##"  <library_controllers><controller id="{n}-skin" name="skin"><skin source="#{n}-mesh">"##, n = name);
    let _ = writeln!(x, "    <bind_shape_matrix>1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1</bind_shape_matrix>");
    let _ = writeln!(
        x,
        r##"    <source id="{n}-joints"><Name_array id="{n}-joints-array" count="{c}">{j}</Name_array><technique_common><accessor source="#{n}-joints-array" count="{c}" stride="1"><param name="JOINT" type="name"/></accessor></technique_common></source>"##,
        n = name,
        c = jc,
        j = joint_names.join(" ")
    );
    let _ = writeln!(
        x,
        r##"    <source id="{n}-bind-poses"><float_array id="{n}-bind-poses-array" count="{c16}">{b}</float_array><technique_common><accessor source="#{n}-bind-poses-array" count="{c}" stride="16"><param name="TRANSFORM" type="float4x4"/></accessor></technique_common></source>"##,
        n = name,
        c = jc,
        c16 = jc * 16,
        b = bind_poses.join(" ")
    );
    let _ = writeln!(
        x,
        r##"    <source id="{n}-weights"><float_array id="{n}-weights-array" count="{c}">{w}</float_array><technique_common><accessor source="#{n}-weights-array" count="{c}" stride="1"><param name="WEIGHT" type="float"/></accessor></technique_common></source>"##,
        n = name,
        c = weights.len(),
        w = floats(&mut weights.iter().copied())
    );
    let _ = writeln!(
        x,
        r##"    <joints><input semantic="JOINT" source="#{n}-joints"/><input semantic="INV_BIND_MATRIX" source="#{n}-bind-poses"/></joints>"##,
        n = name
    );
    let _ = writeln!(
        x,
        r##"    <vertex_weights count="{c}"><input semantic="JOINT" source="#{n}-joints" offset="0"/><input semantic="WEIGHT" source="#{n}-weights" offset="1"/><vcount>{vc}</vcount><v>{v}</v></vertex_weights>"##,
        n = name,
        c = count,
        vc = vcount.join(" "),
        v = v.join(" ")
    );
    let _ = writeln!(x, "  </skin></controller></library_controllers>");

    // Joint hierarchy with translation-only local transforms.
    let _ = writeln!(x, r#"  <library_visual_scenes><visual_scene id="Scene" name="Scene">"#);
    let _ = writeln!(x, r#"    <node id="Armature" name="Armature" type="NODE"><matrix sid="transform">1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1</matrix>"#);
    fn emit(x: &mut String, rig: &SlRig, j: usize, depth: usize) {
        let (name, parent, pos) = rig.joints[j];
        let local = to_sl(pos.sub(parent.map_or(Vec3::ZERO, |p| rig.joints[p].2)));
        let pad = "  ".repeat(depth);
        let _ = writeln!(
            x,
            r#"{pad}<node id="{n}" name="{n}" sid="{n}" type="JOINT"><matrix sid="transform">1 0 0 {:.6} 0 1 0 {:.6} 0 0 1 {:.6} 0 0 0 1</matrix>"#,
            local.x,
            local.y,
            local.z,
            pad = pad,
            n = name
        );
        for c in (0..rig.joints.len()).filter(|&c| rig.joints[c].1 == Some(j)) {
            emit(x, rig, c, depth + 1);
        }
        let _ = writeln!(x, "{}</node>", pad);
    }
    for root in (0..rig.joints.len()).filter(|&j| rig.joints[j].1.is_none()) {
        emit(&mut x, &rig, root, 3);
    }
    let _ = writeln!(x, "    </node>");
    let _ = writeln!(
        x,
        r##"    <node id="{n}-node" name="{n}" type="NODE"><matrix sid="transform">1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1</matrix><instance_controller url="#{n}-skin"><skeleton>#mPelvis</skeleton><bind_material><technique_common><instance_material symbol="clay" target="#clay-material"/></technique_common></bind_material></instance_controller></node>"##,
        n = name
    );
    let _ = writeln!(x, "  </visual_scene></library_visual_scenes>");
    let _ = writeln!(x, r##"  <scene><instance_visual_scene url="#Scene"/></scene>"##);
    let _ = writeln!(x, "</COLLADA>");
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;
    use crate::program::ShapeProgram;

    #[test]
    fn table_is_ordered_unique_and_mirrored() {
        let mut seen = HashSet::new();
        for (name, parent) in JOINTS {
            if let Some(p) = parent {
                assert!(seen.contains(p), "{} listed before its parent {}", name, p);
            }
            assert!(seen.insert(*name), "duplicate {}", name);
            if let Some(right) = crate::compile::swap_side(name) {
                assert!(JOINTS.iter().any(|(n, _)| *n == right), "{} has no mirror {}", name, right);
            }
        }
    }

    fn program(parts: &str) -> Result<CompiledShape, String> {
        let json = format!(r#"{{"skeleton":"second_life","parts":[{}]}}"#, parts);
        compile(&ShapeProgram::from_json(&json).unwrap()).map_err(|e| e.to_string())
    }

    const SPINE: &str = r#"{"kind":"chain","name":"spine","from":[0,1,0],"points":[[0,0.1,0],[0,0.3,0],[0,0.5,0]],"bones":["mPelvis","mTorso","mChest"],"radius":0.15}"#;

    #[test]
    fn validation_explains_wrong_names_and_parenting() {
        let err = program(&format!(r#"{},{{"kind":"blob","name":"h","attach":"mChest","radii":0.1,"bone":"mHeadd"}}"#, SPINE)).unwrap_err();
        assert!(err.contains("did you mean mHead?"), "{}", err);

        let err = program(&format!(
            r#"{},{{"kind":"chain","name":"leg","mirror":true,"attach":"mChest","points":[[0.1,-0.5,0]],"bones":["mKneeLeft"],"radius":0.05}}"#,
            SPINE
        ))
        .unwrap_err();
        assert!(err.contains("attach it under mHipLeft"), "{}", err);

        let err = program(r#"{"kind":"blob","name":"h","from":[0,1,0],"radii":0.1,"bone":"mHead"}"#).unwrap_err();
        assert!(err.contains("rooted at mPelvis"), "{}", err);
    }

    #[test]
    fn helpers_fold_into_sl_ancestors_and_gaps_are_filled() {
        let shape = program(&format!(
            r#"{},
            {{"kind":"blob","name":"head","attach":"mChest","offset":[0,0.15,0],"radii":0.1,"bone":"mHead"}},
            {{"kind":"chain","name":"horn","mirror":true,"attach":"mHead","offset":[0.05,0.05,0],"dir":[0,1,-1],"length":0.2,"segments":2,"radius":[0.02,0.005]}},
            {{"kind":"chain","name":"tail","attach":"mPelvis","at":0,"dir":[0,0,-1],"length":0.6,"segments":3,"bones":["mTail1","mTail2","mTail3"],"radius":[0.06,0.02]}}"#,
            SPINE
        ))
        .unwrap();
        let rig = rig(&shape);
        let names: Vec<&str> = rig.joints.iter().map(|j| j.0).collect();
        // mHead's SL parent mNeck was not modelled; it is inserted on mChest.
        assert!(names.contains(&"mNeck") && names.contains(&"mSpine1") && names.contains(&"mTail3"));
        let neck = names.iter().position(|n| *n == "mNeck").unwrap();
        let chest = names.iter().position(|n| *n == "mChest").unwrap();
        assert_eq!(rig.joints[neck].2, rig.joints[chest].2);
        let horn = shape.bone_index("horn.1.L").unwrap();
        assert_eq!(names[rig.bone_to_joint[horn]], "mHead");
    }
}
