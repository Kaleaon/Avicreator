//! Anatomy-aware shape shifting, including forms with different limb counts.
//!
//! Matching parts interpolate their geometry. Extra anatomy grows or shrinks
//! toward a corresponding attachment. Every intermediate can be remeshed as a
//! skinned GLB; topology changes cannot use fixed-topology glTF morph targets.

use crate::compile::{CompiledShape, Influence};
use crate::sdf::{Aabb, Shape};
use crate::{glsl, preview_in_pose, Preview, ShapeError, SkeletonTarget};
use avicreator_core::{Quat, Vec3};

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub from: String,
    pub to: String,
    pub progress: f32,
}

fn compatible(a: &Shape, b: &Shape) -> bool {
    matches!(
        (a, b),
        (Shape::RoundCone { .. }, Shape::RoundCone { .. })
            | (Shape::Ellipsoid { .. }, Shape::Ellipsoid { .. })
            | (Shape::Slab { .. }, Shape::Slab { .. })
    )
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn interpolate(a: &Shape, b: &Shape, t: f32) -> Shape {
    match (a, b) {
        (
            Shape::RoundCone { a, b, ra, rb },
            Shape::RoundCone {
                a: c,
                b: d,
                ra: rc,
                rb: rd,
            },
        ) => Shape::RoundCone {
            a: a.lerp(*c, t),
            b: b.lerp(*d, t),
            ra: lerp(*ra, *rc, t),
            rb: lerp(*rb, *rd, t),
        },
        (
            Shape::Ellipsoid { center, rot, radii },
            Shape::Ellipsoid {
                center: c,
                rot: r,
                radii: s,
            },
        ) => Shape::Ellipsoid {
            center: center.lerp(*c, t),
            rot: rot.slerp(*r, t),
            radii: radii.lerp(*s, t),
        },
        (
            Shape::Slab {
                a,
                b,
                c,
                half_thickness,
            },
            Shape::Slab {
                a: d,
                b: e,
                c: f,
                half_thickness: h,
            },
        ) => Shape::Slab {
            a: a.lerp(*d, t),
            b: b.lerp(*e, t),
            c: c.lerp(*f, t),
            half_thickness: lerp(*half_thickness, *h, t),
        },
        _ => unreachable!("only compatible primitives interpolate"),
    }
}

/// Collapse a feature around its corresponding attachment, preserving all
/// relative positions and taper rather than fading a disconnected surface.
fn scale_shape(s: &Shape, anchor: Vec3, scale: f32) -> Shape {
    let point = |p: Vec3| anchor.lerp(p, scale);
    match s {
        Shape::RoundCone { a, b, ra, rb } => Shape::RoundCone {
            a: point(*a),
            b: point(*b),
            ra: ra * scale,
            rb: rb * scale,
        },
        Shape::Ellipsoid { center, rot, radii } => Shape::Ellipsoid {
            center: point(*center),
            rot: *rot,
            radii: radii.scale(scale),
        },
        Shape::Slab {
            a,
            b,
            c,
            half_thickness,
        } => Shape::Slab {
            a: point(*a),
            b: point(*b),
            c: point(*c),
            half_thickness: half_thickness * scale,
        },
    }
}

/// Find a shared ancestor, falling back to the nearest shared bone or the other
/// form's centre when conventions differ completely. No naming guesses are made.
fn collapse_anchor(
    shape: &CompiledShape,
    other: &CompiledShape,
    pairs: &[Option<usize>],
    mut bone: Option<usize>,
    point: Vec3,
) -> Vec3 {
    while let Some(i) = bone {
        let b = &shape.bones[i];
        if let Some(j) = pairs[i] {
            return other.bones[j].head;
        }
        bone = b.parent;
    }
    shape
        .bones
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            pairs[i].map(|j| (b.head.sub(point).length_squared(), other.bones[j].head))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(other.bounds.center(), |(_, p)| p)
}

fn part_anchor(
    shape: &CompiledShape,
    other: &CompiledShape,
    pairs: &[Option<usize>],
    i: usize,
) -> Vec3 {
    let bone = shape.influence_bones(i).first().copied().or_else(|| {
        shape.parts[i]
            .group
            .and_then(|g| shape.influence_bones(shape.groups[g].root).first().copied())
    });
    collapse_anchor(shape, other, pairs, bone, shape.parts[i].bounds.center())
}

fn shift_influence(influence: &mut Influence, anchor: Vec3, scale: f32) {
    match influence {
        Influence::Chain { joints, .. } => {
            for p in joints {
                *p = anchor.lerp(*p, scale);
            }
        }
        Influence::Muscle { a, b, .. } => {
            *a = anchor.lerp(*a, scale);
            *b = anchor.lerp(*b, scale);
        }
        _ => {}
    }
}

fn interpolate_influence(a: &mut Influence, b: &Influence, t: f32) {
    match (a, b) {
        (Influence::Chain { joints: a, .. }, Influence::Chain { joints: b, .. })
            if a.len() == b.len() =>
        {
            for (a, b) in a.iter_mut().zip(b) {
                *a = a.lerp(*b, t);
            }
        }
        (Influence::Muscle { a, b, .. }, Influence::Muscle { a: c, b: d, .. }) => {
            *a = a.lerp(*c, t);
            *b = b.lerp(*d, t);
        }
        _ => {}
    }
}

/// Preserve matching named anatomy while introducing/removing extra parts.
/// Exact endpoints retain their original geometry, names and skeleton target.
/// Intermediate rigs are namespaced; arbitrary nested shifts are rejected.
/// Custom forms can establish correspondence with shared part and bone names.
pub fn blend_forms(
    from: &CompiledShape,
    to: &CompiledShape,
    t: f32,
) -> Result<CompiledShape, ShapeError> {
    if !t.is_finite() || !(0.0..=1.0).contains(&t) {
        return Err(ShapeError::Program(
            "shape-shift progress must be finite and within 0..1".into(),
        ));
    }
    if from.transition.is_some() || to.transition.is_some() {
        return Err(ShapeError::Program(
            "shape-shift endpoints must be ordinary compiled forms".into(),
        ));
    }
    if t == 0.0 {
        return Ok(from.clone());
    }
    if t == 1.0 {
        return Ok(to.clone());
    }
    let mut out = from.clone();
    let bone_offset = from.bones.len();
    let part_pairs: Vec<Option<usize>> = from
        .parts
        .iter()
        .map(|part| {
            to.part_index(&part.name).filter(|&j| {
                let target = &to.parts[j];
                part.kind == target.kind
                    && part.op == target.op
                    && part.prims.len() == target.prims.len()
                    && from.prims[part.prims.clone()]
                        .iter()
                        .zip(&to.prims[target.prims.clone()])
                        .all(|(a, b)| compatible(&a.shape, &b.shape))
            })
        })
        .collect();
    let mut bone_pairs: Vec<_> = from.bones.iter().map(|b| to.bone_index(&b.name)).collect();
    let mut reverse_pairs = vec![None; to.bones.len()];
    for (i, pair) in bone_pairs.iter().enumerate() {
        if let Some(j) = pair {
            reverse_pairs[*j] = Some(i);
        }
    }
    // Corresponding parts can bridge different skeleton naming conventions.
    for (i, pair) in part_pairs.iter().enumerate() {
        if let Some(j) = pair {
            let source: Vec<_> = from
                .bones
                .iter()
                .enumerate()
                .filter(|(_, b)| b.part == i)
                .map(|(i, _)| i)
                .collect();
            let target: Vec<_> = to
                .bones
                .iter()
                .enumerate()
                .filter(|(_, b)| b.part == *j)
                .map(|(i, _)| i)
                .collect();
            if source.len() == target.len() {
                for (a, b) in source.into_iter().zip(target) {
                    if bone_pairs[a].is_none() && reverse_pairs[b].is_none() {
                        bone_pairs[a] = Some(b);
                        reverse_pairs[b] = Some(a);
                    }
                }
            }
        }
    }
    let mut target_parts = vec![None; to.parts.len()];
    for (i, part) in from.parts.iter().enumerate() {
        let pair = part_pairs[i];
        if let Some(j) = pair {
            target_parts[j] = Some(i);
            for (a, b) in out.prims[part.prims.clone()]
                .iter_mut()
                .zip(&to.prims[to.parts[j].prims.clone()])
            {
                a.shape = interpolate(&a.shape, &b.shape, t);
                a.bounds = a.shape.bounds();
            }
            interpolate_influence(&mut out.parts[i].influence, &to.parts[j].influence, t);
            out.parts[i].blend = lerp(part.blend, to.parts[j].blend, t);
            out.parts[i].smooth = lerp(part.smooth, to.parts[j].smooth, t);
            for k in 0..3 {
                out.parts[i].color[k] = lerp(part.color[k], to.parts[j].color[k], t);
            }
        } else {
            let anchor = part_anchor(from, to, &bone_pairs, i);
            for prim in &mut out.prims[part.prims.clone()] {
                prim.shape = scale_shape(&prim.shape, anchor, 1.0 - t);
                prim.bounds = prim.shape.bounds();
            }
            shift_influence(&mut out.parts[i].influence, anchor, 1.0 - t);
            out.parts[i].blend *= 1.0 - t;
            out.parts[i].smooth *= 1.0 - t;
        }
        out.parts[i].name = format!("from/{}", part.name);
    }
    for (j, part) in to.parts.iter().enumerate() {
        if target_parts[j].is_some() {
            continue;
        }
        let index = out.parts.len();
        target_parts[j] = Some(index);
        let mut p = part.clone();
        p.name = format!("to/{}", p.name);
        let anchor = part_anchor(to, from, &reverse_pairs, j);
        p.prims = out.prims.len()..out.prims.len() + part.prims.len();
        p.blend *= t;
        p.smooth *= t;
        shift_influence(&mut p.influence, anchor, t);
        match &mut p.influence {
            Influence::Rigid { bone } => *bone += bone_offset,
            Influence::Muscle { bones, .. } => {
                for b in bones {
                    *b += bone_offset;
                }
            }
            Influence::Chain { bones, .. } | Influence::Nearest { bones } => {
                for b in bones {
                    *b += bone_offset;
                }
            }
            _ => {}
        }
        for prim in &to.prims[part.prims.clone()] {
            let mut prim = prim.clone();
            prim.shape = scale_shape(&prim.shape, anchor, t);
            prim.bounds = prim.shape.bounds();
            prim.part = index;
            prim.bone = prim.bone.map(|b| b + bone_offset);
            out.prims.push(prim);
        }
        out.parts.push(p);
    }
    let target_parts: Vec<usize> = target_parts.into_iter().map(Option::unwrap).collect();
    let mut target_groups = vec![0; to.groups.len()];
    for (i, group) in to.groups.iter().enumerate() {
        let root = target_parts[group.root];
        target_groups[i] = if root < from.parts.len() {
            out.parts[root]
                .group
                .expect("matched group root belongs to a group")
        } else {
            let index = out.groups.len();
            out.groups.push(crate::compile::Group {
                root,
                partners: Vec::new(),
                blend: out.parts[root].blend,
            });
            index
        };
    }
    for p in &mut out.parts[from.parts.len()..] {
        p.group = p.group.map(|g| target_groups[g]);
    }
    for group in &mut out.groups[..from.groups.len()] {
        group.blend = out.parts[group.root].blend;
    }
    for (i, group) in to.groups.iter().enumerate() {
        let g = target_groups[i];
        for &p in &group.partners {
            let partner = target_groups[p];
            if partner != g && !out.groups[g].partners.contains(&partner) {
                out.groups[g].partners.push(partner);
            }
        }
    }
    for (i, bone) in from.bones.iter().enumerate() {
        let b = &mut out.bones[i];
        if let Some(j) = bone_pairs[i] {
            let other = &to.bones[j];
            b.head = bone.head.lerp(other.head, t);
            b.tail = bone.tail.lerp(other.tail, t);
            b.rest = bone.rest.slerp(other.rest, t);
            b.pose = Quat::IDENTITY;
        } else {
            let anchor = collapse_anchor(from, to, &bone_pairs, Some(i), bone.head);
            b.head = anchor.lerp(bone.head, 1.0 - t);
            b.tail = anchor.lerp(bone.tail, 1.0 - t);
        }
        b.name = format!("from/{}", bone.name);
    }
    for (i, bone) in to.bones.iter().enumerate() {
        let mut b = bone.clone();
        if let Some(j) = reverse_pairs[i] {
            let other = &from.bones[j];
            b.head = other.head.lerp(bone.head, t);
            b.tail = other.tail.lerp(bone.tail, t);
            b.rest = other.rest.slerp(bone.rest, t);
            b.pose = Quat::IDENTITY;
        } else {
            let anchor = collapse_anchor(to, from, &reverse_pairs, Some(i), bone.head);
            b.head = anchor.lerp(bone.head, t);
            b.tail = anchor.lerp(bone.tail, t);
        }
        b.name = format!("to/{}", bone.name);
        b.part = target_parts[bone.part];
        b.parent = b.parent.map(|p| p + bone_offset);
        out.bones.push(b);
    }
    out.bounds = Aabb::EMPTY;
    for part in &mut out.parts {
        part.bounds = out.prims[part.prims.clone()]
            .iter()
            .fold(Aabb::EMPTY, |b, p| b.union(p.bounds));
        if part.op == crate::program::Op::Union {
            out.bounds = out.bounds.union(part.bounds.inflate(part.blend));
        }
    }
    out.name = format!("{} → {}", from.name, to.name);
    out.skeleton = SkeletonTarget::Custom;
    out.transition = Some(Box::new(Transition {
        from: from.name.clone(),
        to: to.name.clone(),
        progress: t,
    }));
    Ok(out)
}

/// Shared muscle controls carry to the target while proportions keep defaults.
pub fn preview_shift(
    from_text: &str,
    to_text: &str,
    progress: f32,
    pose: Option<&str>,
) -> Result<Preview, ShapeError> {
    let from = preview_in_pose(from_text, &[], pose)?;
    let controls: Vec<_> = ["muscle_mass", "muscle_definition"]
        .iter()
        .filter_map(|&name| {
            from.program
                .params
                .get(name)
                .map(|p| (name.to_string(), p.value()))
        })
        .collect();
    let target_program = crate::ShapeProgram::from_json(to_text)?;
    let controls: Vec<_> = controls
        .into_iter()
        .filter(|(n, _)| target_program.params.contains_key(n))
        .collect();
    let to = preview_in_pose(to_text, &controls, pose)?;
    let shape = blend_forms(&from.shape, &to.shape, progress)?;
    let program = from.program;
    let glsl = glsl::emit_glsl(&shape);
    let mut meta = glsl::meta(
        &program,
        &shape,
        from_text.len() + to_text.len(),
        glsl.len(),
    );
    // Keep the preview framed across both endpoints, including progress 0/1.
    let bounds = from.shape.bounds.union(to.shape.bounds).union(shape.bounds);
    meta["bounds"] = serde_json::json!({"min": bounds.min.to_array(), "max": bounds.max.to_array()});
    meta["shapeshift"] =
        serde_json::json!({"from": from.shape.name, "to": to.shape.name, "progress": progress});
    Ok(Preview {
        program,
        shape,
        glsl,
        meta,
    })
}
