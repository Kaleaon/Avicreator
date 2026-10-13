//! Expands authored parts into primitives, bones and influence rules.

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use avicreator_core::{Quat, Vec3};

use crate::program::{
    ArrayPart, ArrayShape, BlobPart, BoneSpec, ChainPart, Limits, MembranePart, MuscleAnchor, MusclePart, Op, Part, Pivot, Profile, Radii,
    Resolver, Rotation, Scalar, ScopePart, Settings, ShapeProgram, SkeletonTarget, V3,
};
use crate::sdf::{Aabb, Shape};
use crate::ShapeError;

/// Rotation limits in degrees, `[min, max]` about local X (bend), Z (side), Y (twist).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneLimits {
    pub bend: [f32; 2],
    pub side: [f32; 2],
    pub twist: [f32; 2],
}

impl Default for BoneLimits {
    fn default() -> Self {
        Self {
            bend: [-45.0, 45.0],
            side: [-30.0, 30.0],
            twist: [-20.0, 20.0],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub head: Vec3,
    pub tail: Vec3,
    /// Rest orientation in world space: local +Y runs head to tail and local
    /// +X is the bend axis, so bending is a rotation about X.
    pub rest: Quat,
    pub limits: BoneLimits,
    pub part: usize,
    /// World rotation from the authored rest pose to the compiled pose
    /// (identity unless a pose is active).
    pub pose: Quat,
}

/// How a part's bones share influence over the region the part owns.
#[derive(Debug, Clone, PartialEq)]
pub enum Influence {
    /// Geometry only (subtractive parts).
    None,
    /// Bone `i` spans `joints[i]..joints[i + 1]`; neighbours blend near joints.
    Chain { bones: Vec<usize>, joints: Vec<Vec3> },
    /// Everything follows one bone.
    Rigid { bone: usize },
    /// Smooth endpoint-to-endpoint skinning along a muscle's line of action.
    Muscle { bones: [usize; 2], a: Vec3, b: Vec3 },
    /// The two nearest bones by distance to their segments (membranes).
    Nearest { bones: Vec<usize> },
    /// Each primitive follows its own bone (arrays of spikes, plates).
    PerPrimitive,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPart {
    pub name: String,
    pub kind: &'static str,
    pub op: Op,
    /// Smooth-union radius against everything before this part.
    pub blend: f32,
    /// Smooth-union radius between this part's own primitives.
    pub smooth: f32,
    pub prims: Range<usize>,
    pub influence: Influence,
    pub color: [f32; 3],
    pub bounds: Aabb,
    /// Blend group this part belongs to; `None` only for unattached
    /// subtractive parts, which carve the whole shape.
    pub group: Option<usize>,
}

/// A boned part together with its bone-less add-ons (masses, spikes, feet).
/// Parts within a group blend smoothly; a group fillets only into its
/// partners (what it attaches to, or the chains a membrane spans), so
/// siblings such as left and right legs or neighbouring fingers never web.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub root: usize,
    pub partners: Vec<usize>,
    /// Fillet radius against the partners (the root part's blend).
    pub blend: f32,
}

impl CompiledPart {
    /// Softness of the membership blend that decides which part owns a point.
    pub fn membership_softness(&self) -> f32 {
        self.blend.max(0.01) * 0.5
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Primitive {
    pub shape: Shape,
    pub part: usize,
    pub bone: Option<usize>,
    pub bounds: Aabb,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledShape {
    pub name: String,
    pub skeleton: SkeletonTarget,
    pub settings: Settings,
    pub params: BTreeMap<String, f32>,
    pub parts: Vec<CompiledPart>,
    pub prims: Vec<Primitive>,
    pub bones: Vec<Bone>,
    pub groups: Vec<Group>,
    /// Bounds of the union geometry, padded by blend radii.
    pub bounds: Aabb,
    /// Optional field interpolation for forms with different topology.
    pub transition: Option<Box<crate::shapeshift::Transition>>,
}

impl CompiledShape {
    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }

    pub fn part_index(&self, name: &str) -> Option<usize> {
        self.parts.iter().position(|p| p.name == name)
    }
}

/// Reflection across the YZ plane (x -> -x), applied to authored inputs so a
/// mirrored part compiles to the exact mirror image of its `.L` original.
/// Also carries the enclosing scopes' scale, applied to authored lengths.
#[derive(Clone, Copy)]
struct Mirror {
    on: bool,
    suffix: &'static str,
    scale: f32,
}

impl Mirror {
    const NONE: Mirror = Mirror { on: false, suffix: "", scale: 1.0 };
    const LEFT: Mirror = Mirror { on: false, suffix: ".L", scale: 1.0 };
    const RIGHT: Mirror = Mirror { on: true, suffix: ".R", scale: 1.0 };

    fn scaled(self, scale: f32) -> Mirror {
        Mirror { scale: self.scale * scale, ..self }
    }

    /// Positions and directions (scaled with the enclosing scopes).
    fn polar(self, v: Vec3) -> Vec3 {
        let v = v.scale(self.scale);
        if self.on {
            Vec3::new(-v.x, v.y, v.z)
        } else {
            v
        }
    }

    /// Rotation axes transform as pseudo-vectors, which keeps a positive bend
    /// on the right side the mirror image of a positive bend on the left.
    fn axial(self, v: Vec3) -> Vec3 {
        if self.on {
            Vec3::new(v.x, -v.y, -v.z)
        } else {
            v
        }
    }

    /// Name of an explicitly named bone on this side. Left-side names must
    /// carry a side marker so the right side can be derived from them.
    fn bone_name(self, name: &str) -> Result<String, String> {
        match self.suffix {
            "" => Ok(name.to_string()),
            ".L" if swap_side(name).is_some() => Ok(name.to_string()),
            ".R" => swap_side(name).ok_or_else(|| missing_side(name)),
            _ => Err(missing_side(name)),
        }
    }

    /// Side and twist rotations reverse under reflection; bend does not.
    fn limits(self, mut l: BoneLimits) -> BoneLimits {
        if self.on {
            l.side = [-l.side[1], -l.side[0]];
            l.twist = [-l.twist[1], -l.twist[0]];
        }
        l
    }
}

fn missing_side(name: &str) -> String {
    format!(
        "mirrored parts need left-side bone names (e.g. mHipLeft, thigh.L or thigh_L); '{}' has no side marker",
        name
    )
}

/// Swaps a left-side name for its right-side twin (`mHipLeft` -> `mHipRight`,
/// `thigh.L` -> `thigh.R`, `thigh_L` -> `thigh_R`). `None` if no marker.
pub fn swap_side(name: &str) -> Option<String> {
    if let Some(i) = name.rfind("Left") {
        return Some(format!("{}Right{}", &name[..i], &name[i + 4..]));
    }
    for (l, r) in [(".L", ".R"), ("_L", "_R"), (".l", ".r"), ("_l", "_r")] {
        if let Some(stem) = name.strip_suffix(l) {
            return Some(format!("{}{}", stem, r));
        }
    }
    None
}

struct ChainInfo {
    joints: Vec<Vec3>,
    radii: Vec<f32>,
    bones: Vec<usize>,
    /// Pose rotation per segment (follows the parent for shape-only chains).
    poses: Vec<Quat>,
}

struct Compiler<'a> {
    settings: &'a Settings,
    res: Resolver<'a>,
    parts: Vec<CompiledPart>,
    prims: Vec<Primitive>,
    bones: Vec<Bone>,
    bone_lookup: HashMap<String, usize>,
    part_lookup: HashMap<String, usize>,
    chains: HashMap<usize, ChainInfo>,
    /// Per part: the parts it connects to, and whether it roots a group.
    links: Vec<(Vec<usize>, bool)>,
    /// Local rotations of the active pose, by bone name.
    pose: HashMap<String, Quat>,
    /// Endpoint spans in the authored rest pose, with the same parameter values.
    muscle_rest_lengths: HashMap<String, f32>,
}

pub fn compile(program: &ShapeProgram) -> Result<CompiledShape, ShapeError> {
    let s = &program.settings;
    if !(8..=crate::mesher::MAX_RESOLUTION).contains(&s.resolution) {
        return Err(ShapeError::Program(format!("settings.resolution must be between 8 and {}", crate::mesher::MAX_RESOLUTION)));
    }
    if s.triangles.is_some_and(|t| t < 100) {
        return Err(ShapeError::Program("settings.triangles must be at least 100".into()));
    }
    if !(s.blend >= 0.0) || !(0.0..=1.0).contains(&s.joint_blend) {
        return Err(ShapeError::Program(
            "settings.blend must be >= 0 and settings.joint_blend within 0..1".into(),
        ));
    }
    let values = program.param_values();
    let mut c = Compiler {
        settings: s,
        res: Resolver::new(&values),
        parts: Vec::new(),
        prims: Vec::new(),
        bones: Vec::new(),
        bone_lookup: HashMap::new(),
        part_lookup: HashMap::new(),
        chains: HashMap::new(),
        links: Vec::new(),
        pose: HashMap::new(),
        muscle_rest_lengths: HashMap::new(),
    };
    if let Some(name) = &program.pose {
        let spec = program.poses.get(name).ok_or_else(|| {
            ShapeError::Program(format!(
                "unknown pose '{}' (defined: {})",
                name,
                program.poses.keys().cloned().collect::<Vec<_>>().join(", ")
            ))
        })?;
        c.pose = resolve_pose(spec, &c.res).map_err(ShapeError::Program)?;
        let mut rest_program = program.clone();
        rest_program.pose = None;
        let rest = compile(&rest_program)?;
        for part in &rest.parts {
            if let Influence::Muscle { a, b, .. } = &part.influence {
                c.muscle_rest_lengths.insert(part.name.clone(), b.sub(*a).length());
            }
        }
    }

    let mut counter = 0;
    c.parts_in(&program.parts, Mirror::NONE, "", &mut counter)?;

    let mut bounds = Aabb::EMPTY;
    for part in &c.parts {
        if part.op == Op::Union {
            bounds = bounds.union(part.bounds.inflate(part.blend));
        }
    }
    if bounds.is_empty() {
        return Err(ShapeError::Program("program has no additive geometry".into()));
    }

    // Groups in part order: a parent part always precedes what attaches to it.
    let mut groups: Vec<Group> = Vec::new();
    for i in 0..c.parts.len() {
        let (links, own) = &c.links[i];
        if *own {
            let partners = links.iter().filter_map(|&l| c.parts[l].group).collect();
            c.parts[i].group = Some(groups.len());
            groups.push(Group { root: i, partners, blend: c.parts[i].blend });
        } else {
            c.parts[i].group = links.first().and_then(|&l| c.parts[l].group);
        }
    }

    let shape = CompiledShape {
        name: program.name.clone(),
        skeleton: program.skeleton,
        settings: s.clone(),
        params: values.clone(),
        parts: c.parts,
        prims: c.prims,
        bones: c.bones,
        groups,
        bounds,
        transition: None,
    };
    if shape.skeleton == SkeletonTarget::SecondLife {
        crate::sl::validate(&shape).map_err(ShapeError::Program)?;
    }
    Ok(shape)
}

/// Local rotations per bone; left-side entries mirror to the right (side
/// and twist reverse under reflection) unless the right side is given.
fn resolve_pose(spec: &BTreeMap<String, Rotation>, res: &Resolver) -> Result<HashMap<String, Quat>, String> {
    let deg = |s: &Option<Scalar>| -> Result<f32, String> {
        s.as_ref().map_or(Ok(0.0), |s| res.scalar(s).map_err(|e| e.to_string()))
    };
    let mut out = HashMap::new();
    let local = |b: f32, s: f32, t: f32| {
        Quat::from_axis_angle(Vec3::X, b.to_radians())
            .mul(Quat::from_axis_angle(Vec3::Z, s.to_radians()))
            .mul(Quat::from_axis_angle(Vec3::Y, t.to_radians()))
    };
    for (bone, r) in spec {
        let (b, s, t) = (deg(&r.bend)?, deg(&r.side)?, deg(&r.twist)?);
        out.insert(bone.clone(), local(b, s, t));
        if let Some(right) = swap_side(bone) {
            if !spec.contains_key(&right) {
                out.insert(right, local(b, -s, -t));
            }
        }
    }
    Ok(out)
}

fn palette(index: usize) -> [f32; 3] {
    // Evenly spread hues (golden ratio) at a muted, clay-like saturation.
    let h = (index as f32 * 0.618_034 + 0.08).fract() * 6.0;
    let (s, v) = (0.38, 0.86);
    let c = v * s;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

fn positive(v: f32, what: &str) -> Result<f32, String> {
    if v > 0.0 && v.is_finite() {
        Ok(v)
    } else {
        Err(format!("{} must be positive (got {})", what, v))
    }
}

/// Rest frame for a bone along `dir`, rolled so local X follows `bend_ref`
/// (falling back to `fallback` axes, mirrored with the part, when parallel).
fn bone_frame(dir: Vec3, bend_ref: Vec3, fallback: [Vec3; 2]) -> Quat {
    let y = dir.normalize();
    for r in [bend_ref, fallback[0], fallback[1]] {
        let r = r.normalize();
        let x = r.sub(y.scale(r.dot(y)));
        // Require ~8 degrees between the reference and the bone; a nearly
        // parallel reference would leave the roll to rounding noise.
        if x.length_squared() > 0.02 {
            let x = x.normalize();
            return Quat::from_basis(x, y, x.cross(y));
        }
    }
    Quat::IDENTITY
}

/// Frame whose +Z follows `forward`, kept upright where possible.
fn forward_frame(forward: Vec3) -> Quat {
    let z = forward.normalize();
    for up in [Vec3::Y, Vec3::X] {
        let x = up.cross(z);
        if x.length_squared() > 1e-6 {
            let x = x.normalize();
            return Quat::from_basis(x, z.cross(x), z);
        }
    }
    Quat::IDENTITY
}

impl<'a> Compiler<'a> {
    /// Compiles a part list under `frame` (mirror side and scope scale).
    fn parts_in(&mut self, parts: &[Part], frame: Mirror, prefix: &str, counter: &mut usize) -> Result<(), ShapeError> {
        for (i, part) in parts.iter().enumerate() {
            let index = format!("{}{}", prefix, i);
            let label = |m: Mirror, e: String| {
                ShapeError::Program(format!("part {} ('{}{}', {}): {}", index, part.name(), m.suffix, part.kind(), e))
            };
            if part.mirror() && frame.suffix != "" {
                return Err(label(frame, "already mirrored by an enclosing scope".into()));
            }
            let sides: Vec<Mirror> = if part.mirror() {
                vec![Mirror::LEFT.scaled(frame.scale), Mirror::RIGHT.scaled(frame.scale)]
            } else {
                vec![frame]
            };
            let color = palette(*counter);
            *counter += 1;
            for m in sides {
                let result = match part {
                    Part::Chain(p) => self.chain(p, m, color),
                    Part::Blob(p) => self.blob(p, m, color),
                    Part::Membrane(p) => self.membrane(p, m, color),
                    Part::Muscle(p) => self.muscle(p, m, color),
                    Part::Array(p) => self.array(p, m, color),
                    Part::Scope(ScopePart { scale, parts, .. }) => {
                        let s = match scale {
                            Some(s) => self.scalar(s).and_then(|s| positive(s, "scale")).map_err(|e| label(m, e))?,
                            None => 1.0,
                        };
                        self.parts_in(parts, m.scaled(s), &format!("{}.", index), counter)?;
                        Ok(())
                    }
                };
                result.map_err(|e| label(m, e))?;
            }
        }
        Ok(())
    }

    /// A length in the current scope's scale.
    fn len(&self, s: &Scalar, m: Mirror) -> Result<f32, String> {
        Ok(self.scalar(s)? * m.scale)
    }

    fn blend(&self, s: &Option<Scalar>, default: f32, m: Mirror) -> Result<f32, String> {
        Ok(self.opt_scalar(s, default)? * m.scale)
    }

    fn scalar(&self, s: &Scalar) -> Result<f32, String> {
        self.res.scalar(s).map_err(|e| e.to_string())
    }

    fn vec(&self, v: &V3, m: Mirror) -> Result<Vec3, String> {
        self.res.vec3(v).map(|v| m.polar(v)).map_err(|e| e.to_string())
    }

    fn opt_scalar(&self, s: &Option<Scalar>, default: f32) -> Result<f32, String> {
        s.as_ref().map_or(Ok(default), |s| self.scalar(s))
    }

    fn limits(&self, l: &Option<Limits>, m: Mirror) -> Result<BoneLimits, String> {
        let mut out = BoneLimits::default();
        if let Some(l) = l {
            for (src, dst, label) in [
                (&l.bend, &mut out.bend, "bend"),
                (&l.side, &mut out.side, "side"),
                (&l.twist, &mut out.twist, "twist"),
            ] {
                if let Some([lo, hi]) = src {
                    let (lo, hi) = (self.scalar(lo)?, self.scalar(hi)?);
                    if lo > hi {
                        return Err(format!("limits.{} min {} exceeds max {}", label, lo, hi));
                    }
                    *dst = [lo, hi];
                }
            }
        }
        Ok(m.limits(out))
    }

    /// Finds a bone by name, preferring the same-side variant (`name.L`/`name.R`).
    /// A part name resolves to that part's last bone.
    fn find_bone(&self, name: &str, m: Mirror) -> Result<usize, String> {
        let sided = format!("{}{}", name, m.suffix);
        let swapped = if m.on { swap_side(name) } else { None };
        for key in swapped.iter().map(String::as_str).chain([sided.as_str(), name]) {
            if let Some(&b) = self.bone_lookup.get(key) {
                return Ok(b);
            }
        }
        for key in [sided.as_str(), name] {
            if let Some(&p) = self.part_lookup.get(key) {
                if let Some(&b) = self.bones.iter().enumerate().rev().find(|(_, b)| b.part == p).map(|(i, _)| i).as_ref() {
                    return Ok(b);
                }
                return Err(format!("part '{}' has no bones to attach to", key));
            }
        }
        let mut known: Vec<&str> = self.bones.iter().map(|b| b.name.as_str()).collect();
        known.sort_unstable();
        Err(format!(
            "unknown bone or part '{}' (parts can only attach to earlier parts; bones so far: {})",
            name,
            if known.is_empty() { "none".to_string() } else { known.join(", ") }
        ))
    }

    fn find_chain(&self, name: &str, m: Mirror) -> Result<(usize, &ChainInfo), String> {
        let sided = format!("{}{}", name, m.suffix);
        for key in [sided.as_str(), name] {
            if let Some(&p) = self.part_lookup.get(key) {
                return self.chains.get(&p).map(|c| (p, c)).ok_or_else(|| format!("'{}' is not a chain", key));
            }
        }
        Err(format!("unknown chain '{}' (only earlier chains can be referenced)", name))
    }

    /// Start point and parent bone from `from` or `attach`/`at`, plus `offset`.
    fn anchor(
        &self,
        from: &Option<V3>,
        attach: &Option<String>,
        at: &Option<Scalar>,
        offset: &Option<V3>,
        m: Mirror,
    ) -> Result<(Vec3, Vec3, Option<usize>), String> {
        let offset = match offset {
            Some(o) => self.vec(o, m)?,
            None => Vec3::ZERO,
        };
        match (from, attach) {
            (Some(_), Some(_)) => Err("use either \"from\" or \"attach\", not both".into()),
            (Some(f), None) => {
                let p = self.vec(f, m)?;
                Ok((p.add(offset), p, None))
            }
            (None, Some(name)) => {
                let b = self.find_bone(name, m)?;
                let t = self.opt_scalar(at, 1.0)?;
                let bone = &self.bones[b];
                let p = bone.head.lerp(bone.tail, t);
                Ok((p.add(bone.pose.rotate_vec3(offset)), p, Some(b)))
            }
            (None, None) => Ok((offset, offset, None)),
        }
    }

    fn begin_part(&mut self, name: String, m: Mirror) -> Result<usize, String> {
        let full = format!("{}{}", name, m.suffix);
        if self.part_lookup.contains_key(&full) {
            return Err(format!("duplicate part name '{}'", full));
        }
        let index = self.parts.len();
        self.part_lookup.insert(full, index);
        Ok(index)
    }

    #[allow(clippy::too_many_arguments)]
    fn add_bone(
        &mut self,
        name: String,
        parent: Option<usize>,
        head: Vec3,
        tail: Vec3,
        bend_ref: Vec3,
        limits: BoneLimits,
        part: usize,
        pose: Quat,
        m: Mirror,
    ) -> Result<usize, String> {
        if self.bone_lookup.contains_key(&name) {
            return Err(format!("duplicate bone name '{}'", name));
        }
        let index = self.bones.len();
        self.bone_lookup.insert(name.clone(), index);
        self.bones.push(Bone {
            name,
            parent,
            head,
            tail,
            rest: bone_frame(tail.sub(head), bend_ref, [pose.rotate_vec3(m.axial(Vec3::X)), pose.rotate_vec3(m.axial(Vec3::Z))]),
            limits,
            part,
            pose,
        });
        Ok(index)
    }

    fn push_prim(&mut self, shape: Shape, part: usize, bone: Option<usize>) {
        let bounds = shape.bounds();
        self.prims.push(Primitive { shape, part, bone, bounds });
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_part(
        &mut self,
        index: usize,
        name: &str,
        m: Mirror,
        kind: &'static str,
        op: Op,
        blend: f32,
        smooth: f32,
        prim_start: usize,
        influence: Influence,
        color: [f32; 3],
        links: (Vec<usize>, bool),
    ) -> Result<(), String> {
        if blend < 0.0 {
            return Err("blend must be >= 0".into());
        }
        let prims = prim_start..self.prims.len();
        if prims.is_empty() {
            return Err("produced no geometry".into());
        }
        let bounds = self.prims[prims.clone()].iter().fold(Aabb::EMPTY, |b, p| b.union(p.bounds));
        self.parts.push(CompiledPart {
            name: format!("{}{}", name, m.suffix),
            kind,
            op,
            blend,
            smooth,
            prims,
            influence,
            color,
            bounds,
            group: None,
        });
        self.links.push(links);
        debug_assert_eq!(self.parts.len() - 1, index);
        Ok(())
    }

    fn chain(&mut self, p: &ChainPart, m: Mirror, default_color: [f32; 3]) -> Result<(), String> {
        let (start, _, parent) = self.anchor(&p.from, &p.attach, &p.at, &p.offset, m)?;
        let attached: Vec<usize> = parent.map(|b| self.bones[b].part).into_iter().collect();
        // Authored directions are in the rest pose; carry them through the
        // parent's pose rotation.
        let parent_pose = parent.map_or(Quat::IDENTITY, |b| self.bones[b].pose);
        let bend_axis = parent_pose.rotate_vec3(m.axial(match &p.bend_axis {
            Some(v) => self.res.vec3(v).map_err(|e| e.to_string())?,
            None => Vec3::X,
        }));
        if bend_axis.length_squared() < 1e-8 {
            return Err("bend_axis must be non-zero".into());
        }

        let mut joints = vec![start];
        match (&p.points, &p.dir, &p.length) {
            (Some(points), None, None) => {
                if points.is_empty() || points.len() > 64 {
                    return Err("points must list 1 to 64 joint offsets".into());
                }
                if p.segments.is_some() || p.curl.is_some() {
                    return Err("\"segments\" and \"curl\" only apply with \"dir\"/\"length\"".into());
                }
                for pt in points {
                    joints.push(start.add(parent_pose.rotate_vec3(self.vec(pt, m)?)));
                }
            }
            (None, Some(dir), Some(length)) => {
                let dir = parent_pose.rotate_vec3(self.vec(dir, m)?);
                if dir.length_squared() < 1e-8 {
                    return Err("dir must be non-zero".into());
                }
                let length = positive(self.len(length, m)?, "length")?;
                let n = p.segments.unwrap_or(1);
                if !(1..=64).contains(&n) {
                    return Err("segments must be between 1 and 64".into());
                }
                let curl = self.opt_scalar(&p.curl, 0.0)?.to_radians();
                let seg = length / n as f32;
                let mut cur = start;
                for i in 0..n {
                    let turn = if n > 1 { curl * i as f32 / (n - 1) as f32 } else { 0.0 };
                    let d = Quat::from_axis_angle(bend_axis, turn).rotate_vec3(dir.normalize());
                    cur = cur.add(d.scale(seg));
                    joints.push(cur);
                }
            }
            _ => return Err("give either \"points\", or \"dir\" and \"length\"".into()),
        }
        let n = joints.len() - 1;
        for w in joints.windows(2) {
            if w[1].sub(w[0]).length() < 1e-4 {
                return Err("two consecutive joints coincide".into());
            }
        }

        let samples: Vec<f32> = match &p.radius {
            Profile::Constant(r) => vec![self.len(r, m)?],
            Profile::Samples(v) if !v.is_empty() => v.iter().map(|r| self.len(r, m)).collect::<Result<_, _>>()?,
            Profile::Samples(_) => return Err("radius list must not be empty".into()),
        };
        for &r in &samples {
            positive(r, "radius")?;
        }
        let radii: Vec<f32> = (0..=n)
            .map(|j| {
                if samples.len() == 1 {
                    return samples[0];
                }
                let f = j as f32 / n as f32 * (samples.len() - 1) as f32;
                let i = (f.floor() as usize).min(samples.len() - 2);
                samples[i] + (samples[i + 1] - samples[i]) * (f - i as f32)
            })
            .collect();

        let index = self.begin_part(p.name.clone(), m)?;
        let limits = self.limits(&p.limits, m)?;
        let mut bones = Vec::new();
        if p.rigid && (parent.is_none() || p.bones.is_some()) {
            return Err("rigid chains must be attached and cannot name bones".into());
        }
        if let Some(names) = &p.bones {
            if names.len() != n {
                return Err(format!("\"bones\" lists {} names but the chain has {} segments", names.len(), n));
            }
        }
        let mut seg_poses = vec![parent_pose; n];
        if p.op == Op::Union && !p.rigid {
            let names = (0..n)
                .map(|i| match p.bones.as_ref().and_then(|names| names[i].as_deref()) {
                    Some(explicit) => m.bone_name(explicit),
                    None if n > 1 => Ok(format!("{}.{}{}", p.name, i, m.suffix)),
                    None => Ok(format!("{}{}", p.name, m.suffix)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            // Apply the active pose segment by segment: a posed bone turns
            // everything after its head (and its descendants, via `pose`).
            let mut acc = parent_pose;
            let mut axis = bend_axis;
            let mut axes = Vec::with_capacity(n);
            for i in 0..n {
                if let Some(local) = self.pose.get(&names[i]) {
                    let fallback = [acc.rotate_vec3(m.axial(Vec3::X)), acc.rotate_vec3(m.axial(Vec3::Z))];
                    let frame = bone_frame(joints[i + 1].sub(joints[i]), axis, fallback);
                    let world = frame.mul(*local).mul(frame.conjugate());
                    let pivot = joints[i];
                    for j in joints.iter_mut().skip(i + 1) {
                        *j = pivot.add(world.rotate_vec3(j.sub(pivot)));
                    }
                    axis = world.rotate_vec3(axis);
                    acc = world.mul(acc);
                }
                seg_poses[i] = acc;
                axes.push(axis);
            }
            let mut prev = parent;
            for (i, name) in names.into_iter().enumerate() {
                let b = self.add_bone(name, prev, joints[i], joints[i + 1], axes[i], limits, index, seg_poses[i], m)?;
                bones.push(b);
                prev = Some(b);
            }
        }
        let prim_start = self.prims.len();
        for i in 0..n {
            let shape = Shape::RoundCone { a: joints[i], b: joints[i + 1], ra: radii[i], rb: radii[i + 1] };
            self.push_prim(shape, index, bones.get(i).copied());
        }
        let mean_r = radii.iter().sum::<f32>() / radii.len() as f32;
        let influence = match (bones.is_empty(), p.rigid, parent) {
            (false, _, _) => Influence::Chain { bones: bones.clone(), joints: joints.clone() },
            (true, true, Some(b)) if p.op == Op::Union => Influence::Rigid { bone: b },
            _ => Influence::None,
        };
        let blend = self.blend(&p.blend, self.settings.blend, m)?;
        let own = p.op == Op::Union && !p.rigid && (p.separate.unwrap_or(true) || attached.is_empty());
        self.finish_part(index, &p.name, m, "chain", p.op, blend, mean_r * 0.15, prim_start, influence, p.color.unwrap_or(default_color), (attached, own))?;
        self.chains.insert(index, ChainInfo { joints, radii, bones, poses: seg_poses });
        Ok(())
    }

    fn blob(&mut self, p: &BlobPart, m: Mirror, default_color: [f32; 3]) -> Result<(), String> {
        let (center, anchor_point, parent) = self.anchor(&p.from, &p.attach, &p.at, &p.offset, m)?;
        let attached: Vec<usize> = parent.map(|b| self.bones[b].part).into_iter().collect();
        let radii = match &p.radii {
            Radii::Uniform(r) => {
                let r = self.len(r, m)?;
                Vec3::new(r, r, r)
            }
            Radii::PerAxis(v) => self.res.vec3(v).map_err(|e| e.to_string())?.scale(m.scale),
        };
        for r in [radii.x, radii.y, radii.z] {
            positive(r, "radii")?;
        }
        let axis = match &p.axis {
            Some(a) => self.vec(a, m)?,
            None => Vec3::Z,
        };
        if axis.length_squared() < 1e-8 {
            return Err("axis must be non-zero".into());
        }
        // The frame is built in the rest pose, then carried rigidly by the parent's pose.
        let parent_pose = parent.map_or(Quat::IDENTITY, |b| self.bones[b].pose);
        let mut rot = parent_pose.mul(forward_frame(axis));
        let mut center = center;

        let index = self.begin_part(p.name.clone(), m)?;
        let bone_name = match &p.bone {
            None => (p.op == Op::Union).then(|| format!("{}{}", p.name, m.suffix)),
            Some(BoneSpec::Flag(false)) => None,
            Some(BoneSpec::Flag(true)) => Some(format!("{}{}", p.name, m.suffix)),
            Some(BoneSpec::Name(n)) => Some(m.bone_name(n)?),
        };
        let influence = if p.op == Op::Subtract {
            if p.bone.is_some() && bone_name.is_some() {
                return Err("subtractive blobs cannot own a bone".into());
            }
            Influence::None
        } else if let Some(bone_name) = bone_name {
            let mut bend_ref = parent_pose.rotate_vec3(m.axial(match &p.bend_axis {
                Some(v) => self.res.vec3(v).map_err(|e| e.to_string())?,
                None => Vec3::X,
            }));
            let pivot = p.pivot.unwrap_or(if parent.is_some() { Pivot::Attach } else { Pivot::Center });
            let head = if pivot == Pivot::Attach && parent.is_some() { anchor_point } else { center };
            let tail_of = |center: Vec3, rot: Quat| {
                let tail = center.add(rot.rotate_vec3(Vec3::Z).scale(radii.z));
                if tail.sub(head).length() < 1e-4 { head.add(rot.rotate_vec3(Vec3::Y).scale(radii.y)) } else { tail }
            };
            let mut pose = parent_pose;
            if let Some(local) = self.pose.get(&bone_name) {
                let fallback = [parent_pose.rotate_vec3(m.axial(Vec3::X)), parent_pose.rotate_vec3(m.axial(Vec3::Z))];
                let frame = bone_frame(tail_of(center, rot).sub(head), bend_ref, fallback);
                let world = frame.mul(*local).mul(frame.conjugate());
                center = head.add(world.rotate_vec3(center.sub(head)));
                rot = world.mul(rot);
                bend_ref = world.rotate_vec3(bend_ref);
                pose = world.mul(parent_pose);
            }
            let tail = tail_of(center, rot);
            let limits = self.limits(&p.limits, m)?;
            let b = self.add_bone(bone_name, parent, head, tail, bend_ref, limits, index, pose, m)?;
            Influence::Rigid { bone: b }
        } else {
            match parent {
                Some(b) => Influence::Rigid { bone: b },
                None => {
                    return Err("has no bone and is not attached; set \"bone\": true or \"attach\" it".into())
                }
            }
        };
        let prim_start = self.prims.len();
        let bone = match influence {
            Influence::Rigid { bone } => Some(bone),
            _ => None,
        };
        self.push_prim(Shape::Ellipsoid { center, rot, radii }, index, bone);
        let blend = self.blend(&p.blend, self.settings.blend, m)?;
        if p.global && p.op != Op::Subtract {
            return Err("\"global\" only applies to cuts (\"op\": \"subtract\")".into());
        }
        let owns_bone = matches!(influence, Influence::Rigid { bone } if self.bones[bone].part == index);
        let own = owns_bone && (p.separate.unwrap_or(false) || attached.is_empty());
        let links = if p.global { Vec::new() } else { attached };
        self.finish_part(index, &p.name, m, "blob", p.op, blend, 0.0, prim_start, influence, p.color.unwrap_or(default_color), (links, own))
    }

    fn muscle_anchor(&self, p: &MuscleAnchor, m: Mirror) -> Result<(Vec3, usize), String> {
        let at = self.opt_scalar(&p.at, 0.5)?;
        if !(0.0..=1.0).contains(&at) {
            return Err("at must lie within 0..1".into());
        }
        let (point, _, bone) = self.anchor(&None, &Some(p.attach.clone()), &Some(Scalar::Num(at)), &p.offset, m)?;
        let b = bone.expect("attached muscle endpoint has a bone");
        Ok((point, b))
    }

    fn muscle(&mut self, p: &MusclePart, m: Mirror, default_color: [f32; 3]) -> Result<(), String> {
        let (a, origin) = self.muscle_anchor(&p.origin, m).map_err(|e| format!("origin: {}", e))?;
        let (b, insertion) = self.muscle_anchor(&p.insertion, m).map_err(|e| format!("insertion: {}", e))?;
        let length = b.sub(a).length();
        if !length.is_finite() || length <= 1e-5 {
            return Err("origin and insertion must be at least 0.00001 units apart".into());
        }
        let radius = positive(self.len(&p.radius, m)?, "radius")?;
        let tendon = self.opt_scalar(&p.tendon, 0.2)?;
        if !(0.0 < tendon && tendon <= 1.0) {
            return Err("tendon must lie within (0, 1]".into());
        }
        let full_name = format!("{}{}", p.name, m.suffix);
        let rest_length = self.muscle_rest_lengths.get(&full_name).copied().unwrap_or(length);
        // Preserve the swept profile's approximate volume as the attachments
        // move; bound extreme contractions to avoid unbounded bulges.
        let radial_scale = if p.preserve_volume { (rest_length / length).sqrt().clamp(0.5, 2.0) } else { 1.0 };
        let radius = positive(radius * radial_scale, "posed radius")?;
        let index = self.begin_part(p.name.clone(), m)?;
        let prim_start = self.prims.len();
        let profile = |t: f32| radius * (tendon + (1.0 - tendon) * (std::f32::consts::PI * t).sin().max(0.0));
        const SEGMENTS: usize = 12;
        for i in 0..SEGMENTS {
            let t0 = i as f32 / SEGMENTS as f32;
            let t1 = (i + 1) as f32 / SEGMENTS as f32;
            self.push_prim(Shape::RoundCone { a: a.lerp(b, t0), b: a.lerp(b, t1), ra: profile(t0), rb: profile(t1) }, index, None);
        }
        let mut links = vec![self.bones[origin].part];
        if !links.contains(&self.bones[insertion].part) { links.push(self.bones[insertion].part); }
        let blend = self.blend(&p.blend, self.settings.blend * 0.5, m)?;
        self.finish_part(index, &p.name, m, "muscle", Op::Union, blend, 0.0, prim_start,
            Influence::Muscle { bones: [origin, insertion], a, b }, p.color.unwrap_or(default_color), (links, true))
    }

    fn membrane(&mut self, p: &MembranePart, m: Mirror, default_color: [f32; 3]) -> Result<(), String> {
        if p.between.len() < 2 {
            return Err("\"between\" needs at least two chains".into());
        }
        let half = positive(self.len(&p.thickness, m)?, "thickness")? * 0.5;
        let mut rails = Vec::new();
        let mut bones = Vec::new();
        let mut spanned = Vec::new();
        for name in &p.between {
            let (part, chain) = self.find_chain(name, m)?;
            spanned.push(part);
            rails.push(chain.joints.clone());
            for &b in &chain.bones {
                if !bones.contains(&b) {
                    bones.push(b);
                }
            }
        }
        if bones.is_empty() {
            return Err("the chains it spans have no bones".into());
        }

        let index = self.begin_part(p.name.clone(), m)?;
        let prim_start = self.prims.len();
        for pair in rails.windows(2) {
            let k = pair[0].len().max(pair[1].len());
            let a = resample(&pair[0], k);
            let b = resample(&pair[1], k);
            for i in 0..k - 1 {
                for tri in [[a[i], a[i + 1], b[i + 1]], [a[i], b[i + 1], b[i]]] {
                    let area2 = tri[1].sub(tri[0]).cross(tri[2].sub(tri[0])).length();
                    if area2 > 1e-8 {
                        self.push_prim(Shape::Slab { a: tri[0], b: tri[1], c: tri[2], half_thickness: half }, index, None);
                    }
                }
            }
        }
        let blend = self.blend(&p.blend, self.settings.blend, m)?;
        self.finish_part(index, &p.name, m, "membrane", Op::Union, blend, 0.0, prim_start, Influence::Nearest { bones }, p.color.unwrap_or(default_color), (spanned, true))
    }

    fn array(&mut self, p: &ArrayPart, m: Mirror, default_color: [f32; 3]) -> Result<(), String> {
        if !(1..=256).contains(&p.count) {
            return Err("count must be between 1 and 256".into());
        }
        let (t0, t1) = match &p.range {
            Some([a, b]) => (self.scalar(a)?, self.scalar(b)?),
            None => (0.1, 0.9),
        };
        if !(0.0..=1.0).contains(&t0) || !(0.0..=1.0).contains(&t1) {
            return Err("range values must lie within 0..1".into());
        }
        let base_r = positive(self.len(&p.size[0], m)?, "size[0] (radius)")?;
        let height = positive(self.len(&p.size[1], m)?, "size[1] (height)")?;
        let taper = self.opt_scalar(&p.taper, 1.0)?;
        positive(taper, "taper")?;
        let lean = self.opt_scalar(&p.lean, 0.0)?.to_radians();
        let side = match &p.side {
            Some(s) => self.vec(s, m)?,
            None => Vec3::Y,
        };

        let (along, chain) = self.find_chain(&p.along, m)?;
        let (joints, radii, chain_bones, chain_poses) = (chain.joints.clone(), chain.radii.clone(), chain.bones.clone(), chain.poses.clone());
        let lengths: Vec<f32> = joints.windows(2).map(|w| w[1].sub(w[0]).length()).collect();
        let total: f32 = lengths.iter().sum();

        let index = self.begin_part(p.name.clone(), m)?;
        let prim_start = self.prims.len();
        for k in 0..p.count {
            let f = if p.count > 1 { k as f32 / (p.count - 1) as f32 } else { 0.5 };
            let t = t0 + (t1 - t0) * f;
            // Walk the chain to arc-length fraction t.
            let mut dist = t * total;
            let mut seg = 0;
            while seg + 1 < lengths.len() && dist > lengths[seg] {
                dist -= lengths[seg];
                seg += 1;
            }
            let u = (dist / lengths[seg]).clamp(0.0, 1.0);
            let pos = joints[seg].lerp(joints[seg + 1], u);
            let tangent = joints[seg + 1].sub(joints[seg]).normalize();
            let r = radii[seg] + (radii[seg + 1] - radii[seg]) * u;
            let side = chain_poses.get(seg).copied().unwrap_or(Quat::IDENTITY).rotate_vec3(side);
            let mut up = side.sub(tangent.scale(side.dot(tangent)));
            if up.length_squared() < 1e-8 {
                return Err("side points along the chain; choose a direction across it".into());
            }
            up = up.normalize();
            let up = up.scale(lean.cos()).sub(tangent.scale(lean.sin())).normalize();
            let scale = 1.0 + (taper - 1.0) * f;
            let (br, h) = (base_r * scale, height * scale);
            let frame = Quat::from_basis(up.cross(tangent).normalize(), up, tangent);
            let shape = match p.shape {
                ArrayShape::Spike => Shape::RoundCone {
                    a: pos.add(up.scale(r * 0.85)),
                    b: pos.add(up.scale(r + h)),
                    ra: br,
                    rb: br * 0.12,
                },
                ArrayShape::Bump => Shape::Ellipsoid {
                    center: pos.add(up.scale(r * 0.9)),
                    rot: frame,
                    radii: Vec3::new(br, h, br),
                },
                ArrayShape::Plate => Shape::Ellipsoid {
                    center: pos.add(up.scale(r + h * 0.25)),
                    rot: frame,
                    radii: Vec3::new(br * 0.22, h, br),
                },
            };
            let bone = if p.op == Op::Union { chain_bones.get(seg).copied() } else { None };
            self.push_prim(shape, index, bone);
        }
        let influence = if p.op == Op::Union && !chain_bones.is_empty() { Influence::PerPrimitive } else { Influence::None };
        let blend = self.blend(&p.blend, self.settings.blend * 0.5, m)?;
        self.finish_part(index, &p.name, m, "array", p.op, blend, 0.0, prim_start, influence, p.color.unwrap_or(default_color), (vec![along], false))
    }
}

/// Resamples a polyline to `k` points evenly spaced by arc length.
fn resample(points: &[Vec3], k: usize) -> Vec<Vec3> {
    if points.len() == k {
        return points.to_vec();
    }
    let lengths: Vec<f32> = points.windows(2).map(|w| w[1].sub(w[0]).length()).collect();
    let total: f32 = lengths.iter().sum();
    (0..k)
        .map(|i| {
            let mut d = total * i as f32 / (k - 1) as f32;
            for (s, &len) in lengths.iter().enumerate() {
                if d <= len || s + 1 == lengths.len() {
                    return points[s].lerp(points[s + 1], if len > 0.0 { (d / len).min(1.0) } else { 0.0 });
                }
                d -= len;
            }
            *points.last().unwrap()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(json: &str) -> CompiledShape {
        compile(&ShapeProgram::from_json(json).unwrap()).unwrap()
    }

    #[test]
    fn chain_creates_parented_bones_that_follow_params() {
        let json = r#"{
          "params": {"len": 2.0, "curl": 0},
          "parts": [
            {"kind":"chain","name":"body","from":[0,0,0],"dir":[0,0,1],"length":1,"radius":0.3},
            {"kind":"chain","name":"tail","attach":"body","at":0,"dir":[0,0,-1],"length":"$len",
             "segments":4,"curl":"$curl","radius":[0.2,0.02]}
          ]}"#;
        let shape = build(json);
        let names: Vec<&str> = shape.bones.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["body", "tail.0", "tail.1", "tail.2", "tail.3"]);
        assert_eq!(shape.bones[1].parent, Some(0));
        assert_eq!(shape.bones[4].parent, Some(3));
        assert!((shape.bones[4].tail.z + 2.0).abs() < 1e-5);

        // Changing one parameter moves geometry and rig together.
        let mut prog = ShapeProgram::from_json(json).unwrap();
        prog.set_param("len", 3.0).unwrap();
        prog.set_param("curl", 90.0).unwrap();
        let longer = compile(&prog).unwrap();
        assert_eq!(longer.bones.len(), 5);
        let tip = longer.bones[4].tail;
        assert!(tip.y.abs() > 0.5, "curl should lift the tail tip, got {:?}", tip);
        let tail_prim = &longer.prims[longer.parts[1].prims.end - 1];
        if let Shape::RoundCone { b, .. } = tail_prim.shape {
            assert!(b.sub(tip).length() < 1e-5);
        } else {
            panic!("expected a round cone");
        }
    }

    #[test]
    fn mirrored_parts_are_exact_reflections_with_sided_parents() {
        let json = r#"{"parts":[
            {"kind":"chain","name":"spine","from":[0,1,0],"dir":[0,0,1],"length":1,"radius":0.3},
            {"kind":"chain","name":"arm","mirror":true,"attach":"spine","at":0.5,"offset":[0.3,0,0],
             "points":[[0.4,0.1,0.1],[0.8,-0.2,0.2]],"radius":0.08,"limits":{"side":[-10,40]}},
            {"kind":"blob","name":"hand","mirror":true,"attach":"arm","radii":0.1}
        ]}"#;
        let shape = build(json);
        let l = shape.bone_index("arm.1.L").unwrap();
        let r = shape.bone_index("arm.1.R").unwrap();
        let (bl, br) = (&shape.bones[l], &shape.bones[r]);
        assert!((bl.tail.x + br.tail.x).abs() < 1e-5);
        assert!((bl.tail.y - br.tail.y).abs() < 1e-5);
        assert_eq!(br.limits.side, [-40.0, 10.0]);
        assert_eq!(bl.limits.side, [-10.0, 40.0]);
        let hand_r = shape.bone_index("hand.R").unwrap();
        assert_eq!(shape.bones[hand_r].parent, Some(r));

        // Local axes mirror as well: bend axis (X) is a pseudo-vector, Y and Z reflect.
        let xl = bl.rest.rotate_vec3(Vec3::X);
        let xr = br.rest.rotate_vec3(Vec3::X);
        assert!((xl.x - xr.x).abs() < 1e-4 && (xl.y + xr.y).abs() < 1e-4 && (xl.z + xr.z).abs() < 1e-4);
    }

    #[test]
    fn explicit_bone_names_mirror_by_side_marker() {
        assert_eq!(swap_side("mHipLeft").as_deref(), Some("mHipRight"));
        assert_eq!(swap_side("mWing4FanLeft").as_deref(), Some("mWing4FanRight"));
        assert_eq!(swap_side("thigh.L").as_deref(), Some("thigh.R"));
        assert_eq!(swap_side("thigh_L").as_deref(), Some("thigh_R"));
        assert_eq!(swap_side("thigh"), None);

        let json = r#"{"parts":[
            {"kind":"chain","name":"spine","from":[0,1,0],"dir":[0,1,0],"length":0.5,"segments":2,"bones":["mPelvis","mTorso"],"radius":0.15},
            {"kind":"chain","name":"arm","mirror":true,"attach":"mTorso","offset":[0.1,0,0],"points":[[0.3,0,0],[0.55,0,0]],
             "bones":["mShoulderLeft","mElbowLeft"],"radius":0.05},
            {"kind":"chain","name":"finger","mirror":true,"attach":"mElbowLeft","points":[[0.1,0,0]],"bones":[null],"radius":0.01},
            {"kind":"blob","name":"head","attach":"mTorso","offset":[0,0.2,0],"radii":0.1,"bone":"mHead"}
        ]}"#;
        let shape = build(json);
        let right_finger = shape.bone_index("finger.R").unwrap();
        assert_eq!(shape.bones[shape.bones[right_finger].parent.unwrap()].name, "mElbowRight");
        assert!(shape.bone_index("mHead").is_some());

        let err = compile(&ShapeProgram::from_json(r#"{"parts":[
            {"kind":"chain","name":"leg","mirror":true,"from":[0,1,0],"dir":[0,-1,0],"length":1,"bones":["thigh"],"radius":0.1}
        ]}"#).unwrap()).unwrap_err().to_string();
        assert!(err.contains("no side marker"), "{}", err);
    }

    #[test]
    fn errors_name_the_part_and_the_fix() {
        let err = compile(
            &ShapeProgram::from_json(r#"{"parts":[{"kind":"chain","name":"neck","attach":"spine","dir":[0,1,0],"length":1,"radius":0.1}]}"#).unwrap(),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("part 0 ('neck', chain)"), "{}", err);
        assert!(err.contains("unknown bone or part 'spine'"), "{}", err);

        let err = compile(
            &ShapeProgram::from_json(r#"{"parts":[{"kind":"chain","name":"t","from":[0,0,0],"dir":[0,1,0],"radius":0.1}]}"#).unwrap(),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("give either \"points\", or \"dir\" and \"length\""), "{}", err);
    }

    #[test]
    fn scopes_scale_and_mirror_their_parts() {
        let shape = build(
            r#"{"params":{"s":2},"parts":[
                {"kind":"chain","name":"body","from":[0,1,0],"dir":[0,0,1],"length":1,"radius":0.2},
                {"kind":"scope","name":"ear_kit","mirror":true,"scale":"$s","parts":[
                    {"kind":"chain","name":"ear","attach":"body","offset":[0.1,0,0],"dir":[0,1,0],"length":0.1,"radius":0.01},
                    {"kind":"blob","name":"lobe","attach":"ear","at":0,"radii":0.02,"bone":false}
                ]}
            ]}"#,
        );
        let ear = &shape.bones[shape.bone_index("ear.R").unwrap()];
        assert!((ear.head.x + 0.2).abs() < 1e-5, "offset is scaled and mirrored: {:?}", ear.head);
        assert!((ear.tail.sub(ear.head).length() - 0.2).abs() < 1e-5, "length is scaled");
        let lobe = &shape.prims[shape.parts[shape.part_index("lobe.L").unwrap()].prims.start];
        assert!(matches!(lobe.shape, Shape::Ellipsoid { radii, .. } if (radii.x - 0.04).abs() < 1e-6));

        let err = compile(&ShapeProgram::from_json(
            r#"{"parts":[{"kind":"scope","name":"k","mirror":true,"parts":[{"kind":"blob","name":"b","mirror":true,"from":[0,0,0],"radii":1}]}]}"#,
        ).unwrap()).unwrap_err().to_string();
        assert!(err.contains("already mirrored"), "{}", err);
    }

    #[test]
    fn poses_turn_bones_and_everything_attached_mirrored() {
        let json = r#"{
          "poses": {"arms_down": {"arm.L": {"bend": -90}}},
          "parts": [
            {"kind":"chain","name":"body","from":[0,1,0],"dir":[0,1,0],"length":0.5,"radius":0.1},
            {"kind":"chain","name":"arm","mirror":true,"attach":"body","offset":[0.1,0,0],"dir":[1,0,0],"length":0.5,"radius":0.04},
            {"kind":"blob","name":"hand","mirror":true,"attach":"arm","offset":[0.05,0,0],"radii":0.03,"bone":false}
          ]}"#;
        let rest = build(json);
        let mut prog = ShapeProgram::from_json(json).unwrap();
        prog.pose = Some("arms_down".into());
        let posed = compile(&prog).unwrap();
        for side in ["L", "R"] {
            let i = posed.bone_index(&format!("arm.{}", side)).unwrap();
            let (r, p) = (&rest.bones[i], &posed.bones[i]);
            assert!(r.tail.sub(p.head).length() > 0.4);
            assert!((p.tail.y - (p.head.y - 0.5)).abs() < 1e-4, "{} arm hangs straight down: {:?}", side, p.tail);
            // The attached hand's offset turned with the arm.
            let hand = &posed.prims[posed.parts[posed.part_index(&format!("hand.{}", side)).unwrap()].prims.start];
            if let Shape::Ellipsoid { center, .. } = hand.shape {
                assert!((center.y - (p.tail.y - 0.05)).abs() < 1e-4, "{:?}", center);
            }
        }
        let err = {
            let mut prog = ShapeProgram::from_json(json).unwrap();
            prog.pose = Some("nope".into());
            compile(&prog).unwrap_err().to_string()
        };
        assert!(err.contains("unknown pose 'nope'"), "{}", err);
    }

    #[test]
    fn groups_join_add_ons_and_link_attachments() {
        let shape = build(
            r#"{"parts":[
                {"kind":"chain","name":"body","from":[0,1,0],"dir":[0,0,1],"length":1,"radius":0.2},
                {"kind":"blob","name":"belly","attach":"body","at":0.5,"radii":0.25,"bone":false},
                {"kind":"chain","name":"leg","mirror":true,"attach":"body","at":0.2,"offset":[0.1,0,0],"dir":[0,-1,0],"length":0.9,"radius":0.06},
                {"kind":"blob","name":"socket","op":"subtract","attach":"body","radii":0.05},
                {"kind":"blob","name":"hole","op":"subtract","from":[0,0,0],"radii":0.05}
            ]}"#,
        );
        let g = |name: &str| shape.parts[shape.part_index(name).unwrap()].group;
        assert_eq!(g("body"), Some(0));
        assert_eq!(g("belly"), Some(0));
        assert_eq!(g("socket"), Some(0));
        assert_eq!(g("hole"), None);
        let (l, r) = (g("leg.L").unwrap(), g("leg.R").unwrap());
        assert_ne!(l, r);
        assert_eq!(shape.groups[l].partners, vec![0]);
        assert_eq!(shape.groups[r].partners, vec![0]);
    }

    #[test]
    fn membranes_and_arrays_reference_chains() {
        let json = r#"{"parts":[
            {"kind":"chain","name":"a","from":[0,0,0],"points":[[1,0,0],[2,0,0]],"radius":0.05},
            {"kind":"chain","name":"b","from":[0,0,0],"points":[[1,0,-1],[2,0,-2]],"radius":0.05},
            {"kind":"membrane","name":"web","between":["a","b"],"thickness":0.02},
            {"kind":"array","name":"spikes","along":"a","count":5,"size":[0.03,0.1]}
        ]}"#;
        let shape = build(json);
        let web = &shape.parts[shape.part_index("web").unwrap()];
        assert_eq!(web.prims.len(), 3, "first quad degenerates to one triangle at the shared root");
        assert!(matches!(&web.influence, Influence::Nearest { bones } if bones.len() == 4));
        let spikes = &shape.parts[shape.part_index("spikes").unwrap()];
        assert_eq!(spikes.prims.len(), 5);
        assert!(shape.prims[spikes.prims.clone()].iter().all(|p| p.bone.is_some()));
    }
}
