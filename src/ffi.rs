use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_int};
use glam::Vec3;

use crate::morpher::{BaseMesh, Morpher};

#[no_mangle]
pub extern "C" fn avicreator_version() -> *mut c_char {
    let ver = CString::new(env!("CARGO_PKG_VERSION")).unwrap();
    ver.into_raw()
}

#[no_mangle]
pub extern "C" fn avicreator_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

#[no_mangle]
pub extern "C" fn avicreator_morpher_new(
    verts: *const c_float,
    num_verts: usize,
) -> *mut Morpher {
    if verts.is_null() {
        return std::ptr::null_mut();
    }
    let slice = unsafe { std::slice::from_raw_parts(verts, num_verts * 3) };
    let mut vec3_verts = Vec::with_capacity(num_verts);
    for chunk in slice.chunks_exact(3) {
        vec3_verts.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
    }
    let base_mesh = BaseMesh::new(vec3_verts, Vec::new(), Vec::new());
    let morpher = Box::new(Morpher::new(base_mesh));
    Box::into_raw(morpher)
}

#[no_mangle]
pub extern "C" fn avicreator_morpher_free(morpher: *mut Morpher) {
    if !morpher.is_null() {
        unsafe {
            let _ = Box::from_raw(morpher);
        }
    }
}

#[no_mangle]
pub extern "C" fn avicreator_morpher_set_weight(
    morpher: *mut Morpher,
    name: *const c_char,
    weight: c_float,
) {
    if morpher.is_null() || name.is_null() {
        return;
    }
    let morpher = unsafe { &mut *morpher };
    let c_str = unsafe { CStr::from_ptr(name) };
    if let Ok(name_str) = c_str.to_str() {
        morpher.set_weight(name_str, weight);
    }
}

#[no_mangle]
pub extern "C" fn avicreator_morpher_evaluate(
    morpher: *const Morpher,
    out_buf: *mut c_float,
    buf_len: usize,
) -> c_int {
    if morpher.is_null() || out_buf.is_null() {
        return -1;
    }
    let morpher = unsafe { &*morpher };
    let evaluated = morpher.evaluate();
    if buf_len < evaluated.len() * 3 {
        return -2;
    }
    let out_slice = unsafe { std::slice::from_raw_parts_mut(out_buf, evaluated.len() * 3) };
    for (i, v) in evaluated.iter().enumerate() {
        out_slice[i * 3] = v.x;
        out_slice[i * 3 + 1] = v.y;
        out_slice[i * 3 + 2] = v.z;
    }
    0
}
