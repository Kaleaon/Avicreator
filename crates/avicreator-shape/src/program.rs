//! Authored shape programs: the compact JSON an AI (or a person) writes.
//!
//! A program is a list of rig-aware *parts*. Each part carries its shape recipe,
//! the bones it creates and the rule for how those bones influence the surface,
//! so changing one parameter (a tail's length, a horn's curl) updates geometry
//! and rig together. Numbers may be written as `"$name"` expressions over the
//! program's `params`, which keeps edits small and keeps parts consistent.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::ShapeError;

/// A number, or an arithmetic expression over `$params` such as `"$tail_length*0.5"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    Num(f32),
    Expr(String),
}

impl From<f32> for Scalar {
    fn from(v: f32) -> Self {
        Scalar::Num(v)
    }
}

/// A 3-vector whose components are [`Scalar`]s.
pub type V3 = [Scalar; 3];

/// Either a single value used on every axis or one value per axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Radii {
    Uniform(Scalar),
    PerAxis(V3),
}

/// Either a constant radius or a profile sampled evenly from root to tip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Profile {
    Constant(Scalar),
    Samples(Vec<Scalar>),
}

/// A tunable knob. Written as a bare number or as `{"value", "min", "max"}`
/// so editors know a sensible slider range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Param {
    Value(f32),
    Ranged {
        value: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f32>,
    },
}

impl Param {
    pub fn value(&self) -> f32 {
        match self {
            Param::Value(v) => *v,
            Param::Ranged { value, .. } => *value,
        }
    }

    pub fn set_value(&mut self, v: f32) {
        match self {
            Param::Value(x) => *x = v,
            Param::Ranged { value, .. } => *value = v,
        }
    }

    /// Slider range: the declared one, or a range around the current value.
    pub fn range(&self) -> (f32, f32) {
        let v = self.value();
        let (lo, hi) = if v.abs() < 1e-6 {
            (-1.0, 1.0)
        } else if v > 0.0 {
            (0.0, v * 2.0)
        } else {
            (v * 2.0, 0.0)
        };
        match self {
            Param::Value(_) => (lo, hi),
            Param::Ranged { min, max, .. } => (min.unwrap_or(lo), max.unwrap_or(hi)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    #[default]
    Union,
    Subtract,
}

/// Rotation limits in degrees about a bone's local axes:
/// `bend` = local X, `twist` = local Y (along the bone), `side` = local Z.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bend: Option<[Scalar; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<[Scalar; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub twist: Option<[Scalar; 2]>,
}

/// A tapered, optionally curled run of bones: spines, necks, tails, limbs,
/// fingers, horns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainPart {
    pub name: String,
    #[serde(default)]
    pub mirror: bool,
    #[serde(default)]
    pub op: Op,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<V3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attach: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<V3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<V3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curl: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<V3>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bend_axis: Option<V3>,
    /// Explicit names for the chain's bones, root first (`null` keeps the
    /// generated name). Mirrored parts give the left-side name; the right
    /// side swaps `Left`/`.L`/`_L` for `Right`/`.R`/`_R`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bones: Option<Vec<Option<String>>>,
    /// Shape-only chain: no bones of its own; it follows the bone it is
    /// attached to and blends as part of that bone's group (lip contours,
    /// ear rims, jaw lines, muscles).
    #[serde(default)]
    pub rigid: bool,
    /// Blend as its own group, filleting only into what it attaches to, so
    /// siblings never web together (default for chains: limbs, fingers,
    /// tails). `false` blends it smoothly with its parent's whole group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separate: Option<bool>,
    pub radius: Profile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<Limits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
}

/// Where a blob's own bone pivots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pivot {
    /// At the attachment point (a head turning on its neck).
    Attach,
    /// At the blob's centre (an eye turning in its socket).
    Center,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BoneSpec {
    Flag(bool),
    Name(String),
}

/// An ellipsoid mass: heads, torsos, bellies, muscle bulges, eye sockets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlobPart {
    pub name: String,
    #[serde(default)]
    pub mirror: bool,
    #[serde(default)]
    pub op: Op,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<V3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attach: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<V3>,
    pub radii: Radii,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis: Option<V3>,
    /// `true`/`false` to create (or not) a bone named after the part, or a
    /// string to create a bone with that name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bone: Option<BoneSpec>,
    /// Defaults to `attach` for attached blobs, otherwise `center`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot: Option<Pivot>,
    /// For cuts: carve every part (nostrils through several nose parts)
    /// instead of only the group of the bone it is attached to.
    #[serde(default)]
    pub global: bool,
    /// Blend as its own group (see chains). Blobs default to joining their
    /// parent's group, so facial features and muscles blend with neighbours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separate: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bend_axis: Option<V3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<Limits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
}

/// A thin skin stretched between consecutive chains: wings, webbing, fins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MembranePart {
    pub name: String,
    #[serde(default)]
    pub mirror: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend: Option<Scalar>,
    pub between: Vec<String>,
    pub thickness: Scalar,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
}

/// A muscle endpoint on an existing bone. Offsets use authored world axes,
/// like chain/blob offsets, and rotate with the bone in a named pose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MuscleAnchor {
    pub attach: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<V3>,
}

fn default_preserve_volume() -> bool { true }

/// A tapered belly between two rig attachments; creates no extra bones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusclePart {
    pub name: String,
    #[serde(default)]
    pub mirror: bool,
    pub origin: MuscleAnchor,
    pub insertion: MuscleAnchor,
    /// Maximum rest-pose belly radius, scaled by enclosing scopes.
    pub radius: Scalar,
    /// Endpoint radius as a fraction of the belly radius (default 0.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tendon: Option<Scalar>,
    #[serde(default = "default_preserve_volume")]
    pub preserve_volume: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrayShape {
    #[default]
    Spike,
    Bump,
    Plate,
}

/// A repeated feature distributed along a chain: dorsal spikes, plates,
/// scutes, teeth. Each copy follows the bone it sits on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrayPart {
    pub name: String,
    #[serde(default)]
    pub mirror: bool,
    #[serde(default)]
    pub op: Op,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend: Option<Scalar>,
    pub along: String,
    pub count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<[Scalar; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<V3>,
    pub size: [Scalar; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taper: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lean: Option<Scalar>,
    #[serde(default)]
    pub shape: ArrayShape,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
}

/// A list of parts sharing a scale (and optionally mirrored together):
/// a whole head that grows with `$head_size`, or an ear assembly mirrored
/// as one. Scale multiplies offsets, points, lengths, radii, sizes,
/// thicknesses and blend radii of everything inside.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScopePart {
    pub name: String,
    pub mirror: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<Scalar>,
    pub parts: Vec<Part>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScope {
    name: String,
    #[serde(default)]
    mirror: bool,
    #[serde(default)]
    scale: Option<Scalar>,
    parts: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Part {
    Chain(ChainPart),
    Blob(BlobPart),
    Membrane(MembranePart),
    Muscle(MusclePart),
    Array(ArrayPart),
    Scope(ScopePart),
}

impl Part {
    pub fn name(&self) -> &str {
        match self {
            Part::Chain(p) => &p.name,
            Part::Blob(p) => &p.name,
            Part::Membrane(p) => &p.name,
            Part::Muscle(p) => &p.name,
            Part::Array(p) => &p.name,
            Part::Scope(p) => &p.name,
        }
    }

    pub fn mirror(&self) -> bool {
        match self {
            Part::Chain(p) => p.mirror,
            Part::Blob(p) => p.mirror,
            Part::Membrane(p) => p.mirror,
            Part::Muscle(p) => p.mirror,
            Part::Array(p) => p.mirror,
            Part::Scope(p) => p.mirror,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Part::Chain(_) => "chain",
            Part::Blob(_) => "blob",
            Part::Membrane(_) => "membrane",
            Part::Muscle(_) => "muscle",
            Part::Array(_) => "array",
            Part::Scope(_) => "scope",
        }
    }

    fn from_value(index: &str, value: Value) -> Result<Part, ShapeError> {
        let label = value
            .get("name")
            .and_then(Value::as_str)
            .map(|n| format!("part {} ('{}')", index, n))
            .unwrap_or_else(|| format!("part {}", index));
        let mut obj = match value {
            Value::Object(o) => o,
            _ => return Err(ShapeError::Program(format!("{}: expected an object", label))),
        };
        let kind = match obj.remove("kind") {
            Some(Value::String(k)) => k,
            _ => {
                return Err(ShapeError::Program(format!(
                    "{}: missing \"kind\" (one of chain, blob, muscle, membrane, array, scope)",
                    label
                )))
            }
        };
        let rest = Value::Object(obj);
        let wrap = |e: serde_json::Error| ShapeError::Program(format!("{} ({}): {}", label, kind, e));
        Ok(match kind.as_str() {
            "chain" => Part::Chain(serde_json::from_value(rest).map_err(wrap)?),
            "blob" => Part::Blob(serde_json::from_value(rest).map_err(wrap)?),
            "membrane" => Part::Membrane(serde_json::from_value(rest).map_err(wrap)?),
            "muscle" => Part::Muscle(serde_json::from_value(rest).map_err(wrap)?),
            "array" => Part::Array(serde_json::from_value(rest).map_err(wrap)?),
            "scope" => {
                let raw: RawScope = serde_json::from_value(rest).map_err(wrap)?;
                let parts = raw
                    .parts
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| Part::from_value(&format!("{}.{}", index, i), v))
                    .collect::<Result<Vec<_>, _>>()?;
                Part::Scope(ScopePart { name: raw.name, mirror: raw.mirror, scale: raw.scale, parts })
            }
            other => {
                return Err(ShapeError::Program(format!(
                    "{}: unknown kind '{}' (expected chain, blob, muscle, membrane, array or scope)",
                    label, other
                )))
            }
        })
    }
}

/// Global defaults. Every field is optional in JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Settings {
    /// Smooth-union radius used where a part melts into the rest of the body.
    pub blend: f32,
    /// Mesh export resolution: grid cells along the longest bounding-box axis.
    pub resolution: u32,
    /// Fraction of a segment over which neighbouring chain bones share weight.
    pub joint_blend: f32,
    /// Triangle budget: mesh finely, then simplify to at most this many.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triangles: Option<usize>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            blend: 0.05,
            resolution: 96,
            joint_blend: 0.5,
            triangles: None,
        }
    }
}

/// Which skeleton convention the bones must follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkeletonTarget {
    /// Any names and hierarchy.
    #[default]
    Custom,
    /// Second Life (Bento) joint names and parenting; enables `.dae` export.
    SecondLife,
}

/// A rotation in a bone's local frame, in degrees: `bend` about X, `side`
/// about Z, `twist` about Y (along the bone), applied in that order.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rotation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bend: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub twist: Option<Scalar>,
}

/// Named poses: bone name -> rotation. Left-side entries mirror to the right
/// unless the right side is given explicitly.
pub type Poses = BTreeMap<String, BTreeMap<String, Rotation>>;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShapeProgram {
    pub name: String,
    /// Free text: provenance, licence of reference data, authoring notes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub skeleton: SkeletonTarget,
    pub params: BTreeMap<String, Param>,
    pub settings: Settings,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub poses: Poses,
    /// Pose to compile in (`None` = the authored rest pose).
    #[serde(skip)]
    pub pose: Option<String>,
    pub parts: Vec<Part>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProgram {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    skeleton: SkeletonTarget,
    #[serde(default)]
    params: BTreeMap<String, Param>,
    #[serde(default)]
    settings: Settings,
    #[serde(default)]
    poses: Poses,
    parts: Vec<Value>,
}

impl ShapeProgram {
    pub fn from_json(text: &str) -> Result<Self, ShapeError> {
        let raw: RawProgram =
            serde_json::from_str(text).map_err(|e| ShapeError::Program(format!("program: {}", e)))?;
        let parts = raw
            .parts
            .into_iter()
            .enumerate()
            .map(|(i, v)| Part::from_value(&i.to_string(), v))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            name: raw.name.unwrap_or_else(|| "shape".to_string()),
            notes: raw.notes,
            skeleton: raw.skeleton,
            params: raw.params,
            settings: raw.settings,
            poses: raw.poses,
            pose: None,
            parts,
        })
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Overrides a parameter value; unknown names are an error so typos surface.
    pub fn set_param(&mut self, name: &str, value: f32) -> Result<(), ShapeError> {
        match self.params.get_mut(name) {
            Some(p) => {
                p.set_value(value);
                Ok(())
            }
            None => Err(ShapeError::Program(format!(
                "unknown param '{}' (known: {})",
                name,
                self.params.keys().cloned().collect::<Vec<_>>().join(", ")
            ))),
        }
    }

    pub fn param_values(&self) -> BTreeMap<String, f32> {
        self.params.iter().map(|(k, v)| (k.clone(), v.value())).collect()
    }
}

/// Rewrites parameter values inside program text, keeping its formatting:
/// `"name": 1.2` and `"name": {"value": 1.2, ...}` inside `"params"`.
pub fn set_param_values(text: &str, values: &[(String, f32)]) -> Result<String, ShapeError> {
    let params_at = text.find("\"params\"").ok_or_else(|| ShapeError::Program("program has no \"params\"".into()))?;
    let mut out = text.to_string();
    for (name, value) in values {
        let key = format!("\"{}\"", name);
        let rel = out[params_at..].find(&key).ok_or_else(|| ShapeError::Program(format!("param '{}' not found", name)))?;
        let mut i = params_at + rel + key.len();
        let bytes = out.as_bytes();
        while i < bytes.len() && (bytes[i] == b':' || bytes[i].is_ascii_whitespace()) {
            i += 1;
        }
        if bytes.get(i) == Some(&b'{') {
            let rel = out[i..].find("\"value\"").ok_or_else(|| ShapeError::Program(format!("param '{}' has no value", name)))?;
            i += rel + "\"value\"".len();
            let bytes = out.as_bytes();
            while i < bytes.len() && (bytes[i] == b':' || bytes[i].is_ascii_whitespace()) {
                i += 1;
            }
        }
        let bytes = out.as_bytes();
        let mut end = i;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || matches!(bytes[end], b'.' | b'-' | b'+' | b'e' | b'E')) {
            end += 1;
        }
        if end == i {
            return Err(ShapeError::Program(format!("param '{}' is not a plain number", name)));
        }
        let rounded = (value * 1e4).round() / 1e4;
        out.replace_range(i..end, &rounded.to_string());
    }
    ShapeProgram::from_json(&out)?;
    Ok(out)
}

/// Evaluates `$param` expressions: numbers, `$names`, `+ - * /`, unary minus
/// and parentheses.
pub struct Resolver<'a> {
    params: &'a BTreeMap<String, f32>,
}

impl<'a> Resolver<'a> {
    pub fn new(params: &'a BTreeMap<String, f32>) -> Self {
        Self { params }
    }

    pub fn scalar(&self, s: &Scalar) -> Result<f32, ShapeError> {
        match s {
            Scalar::Num(v) => Ok(*v),
            Scalar::Expr(text) => {
                let mut parser = ExprParser {
                    src: text.as_bytes(),
                    pos: 0,
                    params: self.params,
                };
                let v = parser.expr()?;
                parser.skip_ws();
                if parser.pos != parser.src.len() {
                    return Err(parser.error("unexpected trailing input"));
                }
                if !v.is_finite() {
                    return Err(ShapeError::Program(format!("expression '{}' is not finite", text)));
                }
                Ok(v)
            }
        }
    }

    pub fn vec3(&self, v: &V3) -> Result<avicreator_core::Vec3, ShapeError> {
        Ok(avicreator_core::Vec3::new(
            self.scalar(&v[0])?,
            self.scalar(&v[1])?,
            self.scalar(&v[2])?,
        ))
    }
}

struct ExprParser<'a> {
    src: &'a [u8],
    pos: usize,
    params: &'a BTreeMap<String, f32>,
}

impl ExprParser<'_> {
    fn error(&self, msg: &str) -> ShapeError {
        ShapeError::Program(format!(
            "expression '{}': {} at column {}",
            String::from_utf8_lossy(self.src),
            msg,
            self.pos + 1
        ))
    }

    fn skip_ws(&mut self) {
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.src.get(self.pos).copied()
    }

    fn expr(&mut self) -> Result<f32, ShapeError> {
        let mut v = self.term()?;
        while let Some(c) = self.peek() {
            match c {
                b'+' => {
                    self.pos += 1;
                    v += self.term()?;
                }
                b'-' => {
                    self.pos += 1;
                    v -= self.term()?;
                }
                _ => break,
            }
        }
        Ok(v)
    }

    fn term(&mut self) -> Result<f32, ShapeError> {
        let mut v = self.factor()?;
        while let Some(c) = self.peek() {
            match c {
                b'*' => {
                    self.pos += 1;
                    v *= self.factor()?;
                }
                b'/' => {
                    self.pos += 1;
                    v /= self.factor()?;
                }
                _ => break,
            }
        }
        Ok(v)
    }

    fn factor(&mut self) -> Result<f32, ShapeError> {
        match self.peek() {
            Some(b'-') => {
                self.pos += 1;
                Ok(-self.factor()?)
            }
            Some(b'(') => {
                self.pos += 1;
                let v = self.expr()?;
                if self.peek() != Some(b')') {
                    return Err(self.error("expected ')'"));
                }
                self.pos += 1;
                Ok(v)
            }
            Some(b'$') => {
                self.pos += 1;
                let start = self.pos;
                while self.pos < self.src.len()
                    && (self.src[self.pos].is_ascii_alphanumeric() || self.src[self.pos] == b'_')
                {
                    self.pos += 1;
                }
                let name = String::from_utf8_lossy(&self.src[start..self.pos]).to_string();
                self.params
                    .get(&name)
                    .copied()
                    .ok_or_else(|| self.error(&format!("unknown param '${}'", name)))
            }
            Some(c) if c.is_ascii_digit() || c == b'.' => {
                let start = self.pos;
                while self.pos < self.src.len()
                    && (self.src[self.pos].is_ascii_digit()
                        || self.src[self.pos] == b'.'
                        || self.src[self.pos] == b'e'
                        || self.src[self.pos] == b'E'
                        || ((self.src[self.pos] == b'-' || self.src[self.pos] == b'+')
                            && self.pos > start
                            && matches!(self.src[self.pos - 1], b'e' | b'E')))
                {
                    self.pos += 1;
                }
                std::str::from_utf8(&self.src[start..self.pos])
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                    .ok_or_else(|| self.error("invalid number"))
            }
            _ => Err(self.error("expected a number, $param, '-' or '('")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(expr: &str) -> Result<f32, ShapeError> {
        let mut params = BTreeMap::new();
        params.insert("len".to_string(), 2.0);
        params.insert("tail_curl".to_string(), 30.0);
        Resolver::new(&params).scalar(&Scalar::Expr(expr.to_string()))
    }

    #[test]
    fn expressions_follow_precedence_and_params() {
        assert_eq!(eval("$len").unwrap(), 2.0);
        assert_eq!(eval("$len*0.5+1").unwrap(), 2.0);
        assert_eq!(eval("-$tail_curl / 3").unwrap(), -10.0);
        assert_eq!(eval("($len + 1) * 2").unwrap(), 6.0);
        assert_eq!(eval("1e-1*10").unwrap(), 1.0);
        assert!(eval("$nope").unwrap_err().to_string().contains("unknown param '$nope'"));
        assert!(eval("2 +").is_err());
        assert!(eval("1/0").is_err());
    }

    #[test]
    fn parts_report_unknown_fields_with_context() {
        let text = r#"{"parts":[{"kind":"chain","name":"tail","lenght":2,"radius":0.1}]}"#;
        let err = ShapeProgram::from_json(text).unwrap_err().to_string();
        assert!(err.contains("part 0 ('tail') (chain)"), "{}", err);
        assert!(err.contains("lenght"), "{}", err);

        let text = r#"{"parts":[{"kind":"wing","name":"w"}]}"#;
        let err = ShapeProgram::from_json(text).unwrap_err().to_string();
        assert!(err.contains("unknown kind 'wing'"), "{}", err);

        let text = r#"{"parts":[{"kind":"scope","name":"head","scale":2,"parts":[{"kind":"blob","name":"b","radii":1,"oops":1}]}]}"#;
        let err = ShapeProgram::from_json(text).unwrap_err().to_string();
        assert!(err.contains("part 0.0 ('b') (blob)") && err.contains("oops"), "{}", err);
    }

    #[test]
    fn param_values_are_rewritten_in_place() {
        let text = "{\n  \"params\": {\"a\": 1.5, \"b\": {\"value\": 2, \"min\": 1, \"max\": 3}},\n  \"parts\": []\n}";
        let out = set_param_values(text, &[("a".into(), 1.25), ("b".into(), 2.5)]).unwrap();
        assert_eq!(out, "{\n  \"params\": {\"a\": 1.25, \"b\": {\"value\": 2.5, \"min\": 1, \"max\": 3}},\n  \"parts\": []\n}");
    }

    #[test]
    fn params_accept_numbers_or_ranges() {
        let text = r#"{"params":{"a":1.5,"b":{"value":2,"min":1,"max":3}},"parts":[]}"#;
        let mut prog = ShapeProgram::from_json(text).unwrap();
        assert_eq!(prog.params["a"].range(), (0.0, 3.0));
        assert_eq!(prog.params["b"].range(), (1.0, 3.0));
        prog.set_param("b", 2.5).unwrap();
        assert_eq!(prog.param_values()["b"], 2.5);
        assert!(prog.set_param("c", 1.0).is_err());
    }
}
