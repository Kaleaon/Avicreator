#![forbid(unsafe_code)]

pub mod mesh;
pub mod morph;
pub mod vector_math;
pub mod weight_norm;

pub use mesh::{Mesh, Vertex};
pub use morph::{MorphDelta, MorphEvaluator, MorphTarget};
pub use vector_math::{Mat4, Quat, Vec3};
pub use weight_norm::normalize_vertex_weights;
