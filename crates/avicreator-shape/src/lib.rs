#![forbid(unsafe_code)]
//! Rig-aware implicit shape programs.
//!
//! A shape program is a short JSON list of parts (chains, blobs, membranes,
//! arrays). Each part carries its shape recipe, the bones it creates and how
//! those bones influence the surface, so a few parameters drive geometry and
//! rig together. From one compiled program this crate produces:
//!
//! | Layer            | Output                                                   |
//! |------------------|----------------------------------------------------------|
//! | Shape program    | [`ShapeProgram`] (authored JSON with `$params`)          |
//! | Skeleton         | [`Bone`]s with local axes and rotation limits            |
//! | Influence fields | [`CompiledShape::weights`], continuous over space        |
//! | Browser renderer | [`glsl::emit_glsl`] + [`glsl::viewer_html`]              |
//! | Mesh exporter    | [`mesher::mesh`] + [`gltf::write_glb`] (skinned `.glb`)  |
//! | Second Life      | [`sl::write_collada`] (rigged `.dae` on the SL skeleton) |
//!
//! See `docs/shape_programs.md` for the format and the design notes.

pub mod compile;
pub mod field;
pub mod gltf;
pub mod glsl;
pub mod mesher;
pub mod program;
pub mod reference;
pub mod sdf;
pub mod simplify;
pub mod sl;
pub mod studio;

pub use compile::{compile, Bone, BoneLimits, CompiledPart, CompiledShape, Influence};
pub use gltf::{write_glb, GlbOptions};
pub use mesher::{mesh, MeshOptions, MeshResult, MeshStats};
pub use program::{Param, Part, ShapeProgram, SkeletonTarget};

#[derive(Debug, thiserror::Error)]
pub enum ShapeError {
    #[error("{0}")]
    Program(String),
}

/// Everything derived from one program text.
pub struct Preview {
    pub program: ShapeProgram,
    pub shape: CompiledShape,
    pub glsl: String,
    pub meta: serde_json::Value,
}

/// Parses, applies parameter overrides, compiles and emits the preview shader.
pub fn preview(text: &str, overrides: &[(String, f32)]) -> Result<Preview, ShapeError> {
    preview_in_pose(text, overrides, None)
}

/// [`preview`] compiled in one of the program's named poses.
pub fn preview_in_pose(text: &str, overrides: &[(String, f32)], pose: Option<&str>) -> Result<Preview, ShapeError> {
    let mut program = ShapeProgram::from_json(text)?;
    program.pose = pose.map(str::to_string);
    for (name, value) in overrides {
        program.set_param(name, *value)?;
    }
    let shape = compile(&program)?;
    let glsl = glsl::emit_glsl(&shape);
    let meta = glsl::meta(&program, &shape, text.len(), glsl.len());
    Ok(Preview { program, shape, glsl, meta })
}

/// Meshes and skins a shape that targets the Second Life skeleton and writes
/// rigged Collada for the SL uploader. Returns `None` for other skeletons.
pub fn build_sl_dae(shape: &CompiledShape, resolution: Option<u32>) -> Option<(String, MeshStats)> {
    if shape.skeleton != SkeletonTarget::SecondLife {
        return None;
    }
    let result = mesh(shape, mesh_options(shape, resolution));
    Some((sl::write_collada(shape, &result.mesh), result.stats))
}

/// Meshes and skins a compiled shape and packs it as a `.glb`.
pub fn build_glb(shape: &CompiledShape, resolution: Option<u32>, opts: GlbOptions) -> (Vec<u8>, MeshStats) {
    let result = mesh(shape, mesh_options(shape, resolution));
    (write_glb(shape, &result.mesh, opts), result.stats)
}

/// The program's meshing settings, with an optional resolution override.
pub fn mesh_options(shape: &CompiledShape, resolution: Option<u32>) -> MeshOptions {
    let mut opts = MeshOptions::new(resolution.unwrap_or(shape.settings.resolution));
    opts.triangles = shape.settings.triangles;
    opts
}
