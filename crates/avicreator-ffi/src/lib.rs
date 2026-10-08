use avicreator_core::{
    normalize_vertex_weights, Mesh, MorphDelta, MorphEvaluator, MorphTarget, Vec3,
};
use avicreator_schema::{AssetManifest, Skeleton};
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};

pub struct EngineHandle {
    base_mesh: Option<Mesh>,
    morph_targets: HashMap<String, MorphTarget>,
    morph_weights: HashMap<String, f32>,
    deformed_mesh_cache: Option<Mesh>,
}

impl EngineHandle {
    fn new() -> Self {
        Self {
            base_mesh: None,
            morph_targets: HashMap::new(),
            morph_weights: HashMap::new(),
            deformed_mesh_cache: None,
        }
    }
}

/// Creates a new engine instance handle.
/// The returned pointer must be destroyed with `avicreator_engine_destroy`.
#[no_mangle]
pub extern "C" fn avicreator_engine_create() -> *mut EngineHandle {
    Box::into_raw(Box::new(EngineHandle::new()))
}

/// Frees an engine instance handle created by `avicreator_engine_create`.
#[no_mangle]
pub unsafe extern "C" fn avicreator_engine_destroy(engine: *mut EngineHandle) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

/// Explicit string deallocator for strings returned by FFI methods.
#[no_mangle]
pub unsafe extern "C" fn avicreator_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// Explicit f32 buffer deallocator to prevent cross-boundary memory leaks.
#[no_mangle]
pub unsafe extern "C" fn avicreator_free_f32_buffer(ptr: *mut f32, len: usize) {
    if !ptr.is_null() && len > 0 {
        drop(Vec::from_raw_parts(ptr, len, len));
    }
}

/// Explicit u32 buffer deallocator to prevent cross-boundary memory leaks.
#[no_mangle]
pub unsafe extern "C" fn avicreator_free_u32_buffer(ptr: *mut u32, len: usize) {
    if !ptr.is_null() && len > 0 {
        drop(Vec::from_raw_parts(ptr, len, len));
    }
}

/// Sets the base mesh geometry for the avatar engine.
/// Returns 0 on success, -1 if engine pointer is null, -2 if lengths are invalid.
#[no_mangle]
pub unsafe extern "C" fn avicreator_engine_set_mesh(
    engine: *mut EngineHandle,
    positions: *const f32,
    pos_len: usize,
    normals: *const f32,
    norm_len: usize,
    uvs: *const f32,
    uv_len: usize,
    indices: *const u32,
    idx_len: usize,
) -> i32 {
    let engine = match engine.as_mut() {
        Some(e) => e,
        None => return -1,
    };

    if positions.is_null() || pos_len % 3 != 0 {
        return -2;
    }

    let pos_slice = std::slice::from_raw_parts(positions, pos_len);
    let vertex_count = pos_len / 3;

    let mut pos_vec = Vec::with_capacity(vertex_count);
    for chunk in pos_slice.chunks_exact(3) {
        pos_vec.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
    }

    let mut norm_vec = Vec::with_capacity(vertex_count);
    if !normals.is_null() && norm_len == pos_len {
        let norm_slice = std::slice::from_raw_parts(normals, norm_len);
        for chunk in norm_slice.chunks_exact(3) {
            norm_vec.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
        }
    } else {
        norm_vec.resize(vertex_count, Vec3::Y);
    }

    let mut uv_vec = Vec::with_capacity(vertex_count);
    if !uvs.is_null() && uv_len == vertex_count * 2 {
        let uv_slice = std::slice::from_raw_parts(uvs, uv_len);
        for chunk in uv_slice.chunks_exact(2) {
            uv_vec.push([chunk[0], chunk[1]]);
        }
    } else {
        uv_vec.resize(vertex_count, [0.0, 0.0]);
    }

    let idx_vec = if !indices.is_null() && idx_len > 0 {
        std::slice::from_raw_parts(indices, idx_len).to_vec()
    } else {
        Vec::new()
    };

    let mesh = Mesh {
        positions: pos_vec,
        normals: norm_vec,
        uvs: uv_vec,
        indices: idx_vec,
        bone_weights: vec![[1.0, 0.0, 0.0, 0.0]; vertex_count],
        bone_indices: vec![[0, 0, 0, 0]; vertex_count],
    };

    engine.base_mesh = Some(mesh);
    engine.deformed_mesh_cache = None;
    0
}

/// Adds a morph target to the engine.
/// Returns 0 on success, -1 if engine/name/deltas is null, -2 if length invalid.
#[no_mangle]
pub unsafe extern "C" fn avicreator_engine_add_morph_target(
    engine: *mut EngineHandle,
    name: *const c_char,
    deltas: *const f32,
    delta_len: usize,
) -> i32 {
    let engine = match engine.as_mut() {
        Some(e) => e,
        None => return -1,
    };

    if name.is_null() || deltas.is_null() || delta_len % 3 != 0 {
        return -2;
    }

    let c_str = CStr::from_ptr(name);
    let target_name = match c_str.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -2,
    };

    let delta_slice = std::slice::from_raw_parts(deltas, delta_len);
    let mut vec_deltas = Vec::with_capacity(delta_len / 3);
    for chunk in delta_slice.chunks_exact(3) {
        vec_deltas.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
    }

    let target = MorphTarget {
        name: target_name.clone(),
        deltas: MorphDelta {
            position_deltas: vec_deltas,
            normal_deltas: None,
        },
        default_weight: 0.0,
    };

    engine.morph_targets.insert(target_name, target);
    0
}

/// Sets weight for a morph target.
#[no_mangle]
pub unsafe extern "C" fn avicreator_engine_set_morph_weight(
    engine: *mut EngineHandle,
    name: *const c_char,
    weight: f32,
) -> i32 {
    let engine = match engine.as_mut() {
        Some(e) => e,
        None => return -1,
    };

    if name.is_null() {
        return -2;
    }

    let c_str = CStr::from_ptr(name);
    let target_name = match c_str.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -2,
    };

    engine.morph_weights.insert(target_name, weight);
    0
}

/// Computes deformed positions and copies result into output float buffer.
/// Returns 0 on success, -1 if engine is null, -2 if no base mesh, -3 if buffer is too small.
#[no_mangle]
pub unsafe extern "C" fn avicreator_engine_compute_deformed_positions(
    engine: *mut EngineHandle,
    out_positions: *mut f32,
    out_len: usize,
) -> i32 {
    let engine = match engine.as_mut() {
        Some(e) => e,
        None => return -1,
    };

    let base = match &engine.base_mesh {
        Some(m) => m,
        None => return -2,
    };

    let required_len = base.positions.len() * 3;
    if out_positions.is_null() || out_len < required_len {
        return -3;
    }

    let mut active_morphs = Vec::new();
    for (name, weight) in &engine.morph_weights {
        if let Some(target) = engine.morph_targets.get(name) {
            active_morphs.push((target, *weight));
        }
    }

    let evaluator = MorphEvaluator::new();
    let deformed = evaluator.evaluate(base, &active_morphs);

    let out_slice = std::slice::from_raw_parts_mut(out_positions, required_len);
    for (i, p) in deformed.positions.iter().enumerate() {
        out_slice[i * 3] = p.x;
        out_slice[i * 3 + 1] = p.y;
        out_slice[i * 3 + 2] = p.z;
    }

    engine.deformed_mesh_cache = Some(deformed);
    0
}

/// Recalculates normals and fills out_normals buffer.
#[no_mangle]
pub unsafe extern "C" fn avicreator_engine_recalculate_normals(
    engine: *mut EngineHandle,
    out_normals: *mut f32,
    out_len: usize,
) -> i32 {
    let engine = match engine.as_mut() {
        Some(e) => e,
        None => return -1,
    };

    let mesh = match &mut engine.deformed_mesh_cache {
        Some(m) => m,
        None => match &engine.base_mesh {
            Some(bm) => bm,
            None => return -2,
        },
    };

    let required_len = mesh.normals.len() * 3;
    if out_normals.is_null() || out_len < required_len {
        return -3;
    }

    let out_slice = std::slice::from_raw_parts_mut(out_normals, required_len);
    for (i, n) in mesh.normals.iter().enumerate() {
        out_slice[i * 3] = n.x;
        out_slice[i * 3 + 1] = n.y;
        out_slice[i * 3 + 2] = n.z;
    }

    0
}

/// Gets the vertex count of the current base mesh.
#[no_mangle]
pub unsafe extern "C" fn avicreator_engine_get_vertex_count(engine: *mut EngineHandle) -> usize {
    let engine = match engine.as_ref() {
        Some(e) => e,
        None => return 0,
    };

    engine.base_mesh.as_ref().map_or(0, |m| m.vertex_count())
}

/// Gets the index count of the current base mesh.
#[no_mangle]
pub unsafe extern "C" fn avicreator_engine_get_index_count(engine: *mut EngineHandle) -> usize {
    let engine = match engine.as_ref() {
        Some(e) => e,
        None => return 0,
    };

    engine.base_mesh.as_ref().map_or(0, |m| m.index_count())
}

/// Normalizes vertex weights buffer in place.
#[no_mangle]
pub unsafe extern "C" fn avicreator_normalize_weights(
    weights: *mut f32,
    len: usize,
    epsilon: f32,
) -> i32 {
    if weights.is_null() || len % 4 != 0 {
        return -1;
    }

    let count = len / 4;
    let slice = std::slice::from_raw_parts_mut(weights as *mut [f32; 4], count);
    normalize_vertex_weights(slice, epsilon);
    0
}

/// Parses skeleton JSON string and returns canonical JSON representation string.
/// The returned string pointer must be deallocated using `avicreator_free_string`.
/// Returns null pointer if parsing fails or input is invalid.
#[no_mangle]
pub unsafe extern "C" fn avicreator_parse_skeleton_json(json_str: *const c_char) -> *mut c_char {
    if json_str.is_null() {
        return std::ptr::null_mut();
    }

    let c_str = CStr::from_ptr(json_str);
    let s = match c_str.to_str() {
        Ok(val) => val,
        Err(_) => return std::ptr::null_mut(),
    };

    match Skeleton::from_json(s) {
        Ok(skel) => match skel.to_json() {
            Ok(json) => CString::new(json).map_or(std::ptr::null_mut(), |c| c.into_raw()),
            Err(_) => std::ptr::null_mut(),
        },
        Err(_) => std::ptr::null_mut(),
    }
}

/// Parses asset manifest JSON string and returns canonical JSON representation string.
/// The returned string pointer must be deallocated using `avicreator_free_string`.
/// Returns null pointer if parsing fails or input is invalid.
#[no_mangle]
pub unsafe extern "C" fn avicreator_parse_manifest_json(json_str: *const c_char) -> *mut c_char {
    if json_str.is_null() {
        return std::ptr::null_mut();
    }

    let c_str = CStr::from_ptr(json_str);
    let s = match c_str.to_str() {
        Ok(val) => val,
        Err(_) => return std::ptr::null_mut(),
    };

    match AssetManifest::from_json(s) {
        Ok(manifest) => match manifest.to_json() {
            Ok(json) => CString::new(json).map_or(std::ptr::null_mut(), |c| c.into_raw()),
            Err(_) => std::ptr::null_mut(),
        },
        Err(_) => std::ptr::null_mut(),
    }
}

/// Validates skeleton JSON. Returns 1 if valid, 0 if invalid.
#[no_mangle]
pub unsafe extern "C" fn avicreator_validate_skeleton_json(json_str: *const c_char) -> i32 {
    if json_str.is_null() {
        return 0;
    }

    let c_str = CStr::from_ptr(json_str);
    let s = match c_str.to_str() {
        Ok(val) => val,
        Err(_) => return 0,
    };

    if Skeleton::from_json(s).is_ok() {
        1
    } else {
        0
    }
}

/// Validates manifest JSON. Returns 1 if valid, 0 if invalid.
#[no_mangle]
pub unsafe extern "C" fn avicreator_validate_manifest_json(json_str: *const c_char) -> i32 {
    if json_str.is_null() {
        return 0;
    }

    let c_str = CStr::from_ptr(json_str);
    let s = match c_str.to_str() {
        Ok(val) => val,
        Err(_) => return 0,
    };

    if AssetManifest::from_json(s).is_ok() {
        1
    } else {
        0
    }
}

/// Returns C string version of Avicreator library.
/// Caller must free string using `avicreator_free_string`.
#[no_mangle]
pub extern "C" fn avicreator_version() -> *mut c_char {
    let ver = env!("CARGO_PKG_VERSION");
    CString::new(ver).unwrap_or_default().into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_c_ffi_lifecycle() {
        unsafe {
            let engine = avicreator_engine_create();
            assert!(!engine.is_null());

            let positions = vec![0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
            let indices = vec![0u32, 1, 2];

            let res = avicreator_engine_set_mesh(
                engine,
                positions.as_ptr(),
                positions.len(),
                ptr::null(),
                0,
                ptr::null(),
                0,
                indices.as_ptr(),
                indices.len(),
            );
            assert_eq!(res, 0);

            assert_eq!(avicreator_engine_get_vertex_count(engine), 3);
            assert_eq!(avicreator_engine_get_index_count(engine), 3);

            let smile_name = CString::new("smile").unwrap();
            let deltas = vec![0.0f32, 0.1, 0.0, 0.0, 0.1, 0.0, 0.0, 0.1, 0.0];

            let morph_res = avicreator_engine_add_morph_target(
                engine,
                smile_name.as_ptr(),
                deltas.as_ptr(),
                deltas.len(),
            );
            assert_eq!(morph_res, 0);

            let weight_res = avicreator_engine_set_morph_weight(engine, smile_name.as_ptr(), 1.0);
            assert_eq!(weight_res, 0);

            let mut out_positions = vec![0.0f32; 9];
            let comp_res = avicreator_engine_compute_deformed_positions(
                engine,
                out_positions.as_mut_ptr(),
                out_positions.len(),
            );
            assert_eq!(comp_res, 0);

            // V0 y was 0.0, added 0.1 delta * 1.0 = 0.1
            assert!((out_positions[1] - 0.1).abs() < 1e-5);

            let ver = avicreator_version();
            assert!(!ver.is_null());
            let ver_str = CStr::from_ptr(ver).to_str().unwrap();
            assert_eq!(ver_str, "0.1.0");
            avicreator_free_string(ver);

            avicreator_engine_destroy(engine);
        }
    }

    #[test]
    fn test_c_ffi_json_parsing() {
        unsafe {
            // Skeleton test
            let skel_raw = r#"{"name": "Biped", "nodes": [{"id": 0, "name": "Hips", "translation": [0.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]}], "root_indices": [0]}"#;
            let c_skel = CString::new(skel_raw).unwrap();
            assert_eq!(avicreator_validate_skeleton_json(c_skel.as_ptr()), 1);

            let parsed_skel_ptr = avicreator_parse_skeleton_json(c_skel.as_ptr());
            assert!(!parsed_skel_ptr.is_null());
            let parsed_skel_str = CStr::from_ptr(parsed_skel_ptr).to_str().unwrap();
            assert!(parsed_skel_str.contains("Biped"));
            assert!(parsed_skel_str.contains("Hips"));
            avicreator_free_string(parsed_skel_ptr);

            // Manifest test
            let manifest_raw = r#"{"id": "c1", "name": "Char", "version": "1.0", "category": "character", "skeleton_type": "biped"}"#;
            let c_manifest = CString::new(manifest_raw).unwrap();
            assert_eq!(avicreator_validate_manifest_json(c_manifest.as_ptr()), 1);

            let parsed_man_ptr = avicreator_parse_manifest_json(c_manifest.as_ptr());
            assert!(!parsed_man_ptr.is_null());
            let parsed_man_str = CStr::from_ptr(parsed_man_ptr).to_str().unwrap();
            assert!(parsed_man_str.contains("Char"));
            assert!(parsed_man_str.contains("biped"));
            avicreator_free_string(parsed_man_ptr);
        }
    }
}
