#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use numpy::{PyArray1, PyArray2, PyArrayMethods};
#[cfg(feature = "python")]
use glam::Vec3;
#[cfg(feature = "python")]
use std::collections::HashMap;

#[cfg(feature = "python")]
use crate::{
    fitting::{FittingConfig, Fitter},
    morpher::{BaseMesh, MorphTarget, Morpher, SparseDelta},
    rigging::{Bone, BonePose, Skeleton, VertexWeight},
    spatial::{BVHTree, KDTree},
};

fn vec3_to_pyarray<'py>(py: Python<'py>, vec: Vec<Vec3>) -> Bound<'py, PyArray2<f32>> {
    let num_verts = vec.len();
    let flat_slice: &[f32] = bytemuck::cast_slice(&vec);
    let arr1 = PyArray1::from_vec_bound(py, flat_slice.to_vec());
    arr1.reshape((num_verts, 3)).unwrap()
}

#[cfg(feature = "python")]
#[pyclass(name = "BVHTree")]
pub struct PyBVHTree {
    inner: BVHTree,
}

#[cfg(feature = "python")]
#[pymethods]
impl PyBVHTree {
    #[staticmethod]
    pub fn from_polygons(verts: Vec<[f32; 3]>, polys: Vec<[usize; 3]>) -> PyResult<Self> {
        let vec3_verts: Vec<Vec3> = verts.iter().map(|v| Vec3::from_slice(v)).collect();
        let inner = BVHTree::FromPolygons(&vec3_verts, &polys);
        Ok(Self { inner })
    }

    #[staticmethod]
    #[allow(non_snake_case)]
    pub fn FromPolygons(verts: Vec<[f32; 3]>, polys: Vec<[usize; 3]>) -> PyResult<Self> {
        Self::from_polygons(verts, polys)
    }

    pub fn ray_cast(
        &self,
        origin: [f32; 3],
        direction: [f32; 3],
        distance: Option<f32>,
    ) -> Option<([f32; 3], [f32; 3], usize, f32)> {
        let max_d = distance.unwrap_or(0.0);
        let hit = self
            .inner
            .ray_cast(Vec3::from_slice(&origin), Vec3::from_slice(&direction), max_d)?;
        Some((
            hit.point.to_array(),
            hit.normal.to_array(),
            hit.face_index,
            hit.distance,
        ))
    }

    pub fn find_nearest(
        &self,
        point: [f32; 3],
        distance: Option<f32>,
    ) -> Option<([f32; 3], [f32; 3], usize, f32)> {
        let max_d = distance.unwrap_or(0.0);
        let hit = self
            .inner
            .find_nearest(Vec3::from_slice(&point), max_d)?;
        Some((
            hit.point.to_array(),
            hit.normal.to_array(),
            hit.face_index,
            hit.distance,
        ))
    }
}

#[cfg(feature = "python")]
#[pyclass(name = "KDTree")]
pub struct PyKDTree {
    inner: KDTree,
}

#[cfg(feature = "python")]
#[pymethods]
impl PyKDTree {
    #[new]
    pub fn new(size_hint: usize) -> Self {
        Self {
            inner: KDTree::new(size_hint),
        }
    }

    #[staticmethod]
    pub fn build(points: Vec<[f32; 3]>) -> Self {
        let vec3_pts: Vec<Vec3> = points.iter().map(|p| Vec3::from_slice(p)).collect();
        Self {
            inner: KDTree::build(&vec3_pts),
        }
    }

    pub fn find(&self, target: [f32; 3]) -> Option<([f32; 3], usize, f32)> {
        let (pt, idx, dist) = self.inner.find_nearest(Vec3::from_slice(&target))?;
        Some((pt.to_array(), idx, dist))
    }

    pub fn find_n(&self, target: [f32; 3], n: usize) -> Vec<([f32; 3], usize, f32)> {
        self.inner
            .find_n_nearest(Vec3::from_slice(&target), n)
            .into_iter()
            .map(|(pt, idx, dist)| (pt.to_array(), idx, dist))
            .collect()
    }

    pub fn find_range(&self, target: [f32; 3], radius: f32) -> Vec<([f32; 3], usize, f32)> {
        self.inner
            .find_range(Vec3::from_slice(&target), radius)
            .into_iter()
            .map(|(pt, idx, dist)| (pt.to_array(), idx, dist))
            .collect()
    }
}

#[cfg(feature = "python")]
#[pyclass(name = "Morpher")]
pub struct PyMorpher {
    inner: Morpher,
}

#[cfg(feature = "python")]
#[pymethods]
impl PyMorpher {
    #[new]
    pub fn new(vertices: Vec<[f32; 3]>, polygons: Option<Vec<[usize; 3]>>) -> Self {
        let vec3_verts: Vec<Vec3> = vertices.iter().map(|v| Vec3::from_slice(v)).collect();
        let polys = polygons.unwrap_or_default();
        let base_mesh = BaseMesh::new(vec3_verts, Vec::new(), polys);
        Self {
            inner: Morpher::new(base_mesh),
        }
    }

    pub fn add_morph_sparse(
        &mut self,
        name: String,
        min_val: f32,
        max_val: f32,
        default_val: f32,
        indices: Vec<usize>,
        offsets: Vec<[f32; 3]>,
    ) {
        let deltas: Vec<SparseDelta> = indices
            .into_iter()
            .zip(offsets.into_iter())
            .map(|(idx, off)| SparseDelta {
                vertex_index: idx,
                offset: Vec3::from_slice(&off),
            })
            .collect();
        let target = MorphTarget::new_sparse(name, min_val, max_val, default_val, deltas);
        self.inner.add_morph(target);
    }

    pub fn add_morph_dense(
        &mut self,
        name: String,
        min_val: f32,
        max_val: f32,
        default_val: f32,
        offsets: Vec<[f32; 3]>,
    ) {
        let deltas: Vec<Vec3> = offsets.iter().map(|o| Vec3::from_slice(o)).collect();
        let target = MorphTarget::new_dense(name, min_val, max_val, default_val, deltas);
        self.inner.add_morph(target);
    }

    pub fn set_weight(&mut self, name: &str, weight: f32) {
        self.inner.set_weight(name, weight);
    }

    pub fn get_weight(&self, name: &str) -> f32 {
        self.inner.get_weight(name)
    }

    pub fn reset_weights(&mut self) {
        self.inner.reset_weights();
    }

    pub fn evaluate<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
        let evaluated = self.inner.evaluate();
        vec3_to_pyarray(py, evaluated)
    }

    pub fn mix_presets(
        &mut self,
        preset_a: HashMap<String, f32>,
        preset_b: HashMap<String, f32>,
        mix_factor: f32,
    ) {
        self.inner.mix_presets(&preset_a, &preset_b, mix_factor);
    }
}

#[cfg(feature = "python")]
#[pyclass(name = "Fitter")]
pub struct PyFitter {
    inner: Fitter,
}

#[cfg(feature = "python")]
#[pymethods]
impl PyFitter {
    #[new]
    pub fn new(body_vertices: Vec<[f32; 3]>, body_polygons: Vec<[usize; 3]>) -> Self {
        let vec3_verts: Vec<Vec3> = body_vertices.iter().map(|v| Vec3::from_slice(v)).collect();
        let inner = Fitter::new(vec3_verts, &body_polygons);
        Self { inner }
    }

    pub fn fit_asset<'py>(
        &self,
        py: Python<'py>,
        asset_vertices: Vec<[f32; 3]>,
        max_distance: Option<f32>,
        smoothing_iterations: Option<usize>,
    ) -> Bound<'py, PyArray2<f32>> {
        let vec3_asset: Vec<Vec3> = asset_vertices.iter().map(|v| Vec3::from_slice(v)).collect();
        let config = FittingConfig {
            max_distance: max_distance.unwrap_or(0.1),
            smoothing_iterations: smoothing_iterations.unwrap_or(2),
            ..Default::default()
        };
        let fitted = self.inner.fit_asset(&vec3_asset, &config);
        vec3_to_pyarray(py, fitted)
    }
}

#[cfg(feature = "python")]
#[pyclass(name = "Rigger")]
pub struct PyRigger {
    bones: Vec<Bone>,
}

#[cfg(feature = "python")]
#[pymethods]
impl PyRigger {
    #[new]
    pub fn new() -> Self {
        Self { bones: Vec::new() }
    }

    pub fn add_bone(
        &mut self,
        name: String,
        head: [f32; 3],
        tail: [f32; 3],
        parent: Option<String>,
    ) {
        self.bones.push(Bone::new(
            name,
            Vec3::from_slice(&head),
            Vec3::from_slice(&tail),
            parent,
        ));
    }

    pub fn skin_vertices<'py>(
        &self,
        py: Python<'py>,
        vertices: Vec<[f32; 3]>,
        weights: Vec<Vec<(usize, f32)>>,
        poses: Option<HashMap<String, [f32; 3]>>,
    ) -> Bound<'py, PyArray2<f32>> {
        let vec3_verts: Vec<Vec3> = vertices.iter().map(|v| Vec3::from_slice(v)).collect();
        let skeleton = Skeleton::new(self.bones.clone());

        let mut pose_map = HashMap::new();
        if let Some(p_map) = poses {
            for (k, v) in p_map {
                pose_map.insert(
                    k,
                    BonePose {
                        translation: Vec3::from_slice(&v),
                        ..Default::default()
                    },
                );
            }
        }

        let pose_matrices = skeleton.compute_pose_matrices(&pose_map);

        let v_weights: Vec<Vec<VertexWeight>> = weights
            .into_iter()
            .map(|w_list| {
                w_list
                    .into_iter()
                    .map(|(b_idx, w)| VertexWeight {
                        bone_index: b_idx,
                        weight: w,
                    })
                    .collect()
            })
            .collect();

        let skinned = skeleton.skin_vertices(&vec3_verts, &v_weights, &pose_matrices);
        vec3_to_pyarray(py, skinned)
    }
}

#[cfg(feature = "python")]
#[pyfunction]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(feature = "python")]
#[pymodule]
fn libavicreator(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBVHTree>()?;
    m.add_class::<PyKDTree>()?;
    m.add_class::<PyMorpher>()?;
    m.add_class::<PyFitter>()?;
    m.add_class::<PyRigger>()?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
