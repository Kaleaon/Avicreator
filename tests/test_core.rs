use libavicreator::{
    math::{Ray, Triangle},
    morpher::{BaseMesh, MorphDelta, MorphTarget, Morpher, SparseDelta},
    rigging::{Bone, BonePose, Skeleton, VertexWeight},
    spatial::{BVHTree, KDTree},
    FitCalc, Fitter, GltfIO,
};
use glam::Vec3;
use std::collections::HashMap;
use std::time::Instant;

#[test]
fn test_bvh_ray_cast() {
    let verts = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    let polys = vec![[0, 1, 2]];
    let bvh = BVHTree::FromPolygons(&verts, &polys);

    let hit = bvh.ray_cast(Vec3::new(0.2, 0.2, 1.0), Vec3::new(0.0, 0.0, -1.0), 10.0);
    assert!(hit.is_some());
    let hit = hit.unwrap();
    assert!((hit.point.z - 0.0).abs() < 1e-5);
    assert_eq!(hit.face_index, 0);
}

#[test]
fn test_kdtree_search() {
    let points = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 2.0, 0.0),
    ];
    let kdtree = KDTree::build(&points);

    let (pt, idx, dist) = kdtree.find_nearest(Vec3::new(0.1, 0.1, 0.0)).unwrap();
    assert_eq!(idx, 0);
    assert!(dist < 0.2);
}

#[test]
fn test_morpher_performance_50k_vertices() {
    let num_verts = 50_000;
    let mut vertices = Vec::with_capacity(num_verts);
    for i in 0..num_verts {
        vertices.push(Vec3::new(i as f32 * 0.01, (i % 100) as f32 * 0.01, 0.0));
    }
    let base_mesh = BaseMesh::new(vertices, Vec::new(), Vec::new());
    let mut morpher = Morpher::new(base_mesh);

    // Add 10 dense morph targets
    for m in 0..10 {
        let deltas: Vec<Vec3> = (0..num_verts)
            .map(|i| Vec3::new(0.001 * (m as f32 + 1.0), 0.0, 0.001 * (i % 10) as f32))
            .collect();
        let target = MorphTarget::new_dense(format!("morph_{}", m), -1.0, 1.0, 0.0, deltas);
        morpher.add_morph(target);
        morpher.set_weight(&format!("morph_{}", m), 0.5);
    }

    // Warm up
    let _ = morpher.evaluate();

    // Benchmark single frame evaluation
    let start = Instant::now();
    let evaluated = morpher.evaluate();
    let elapsed = start.elapsed();

    assert_eq!(evaluated.len(), num_verts);
    println!("Morpher 50k vertex evaluation time: {:?}", elapsed);
    let max_allowed = if cfg!(debug_assertions) { 0.050 } else { 0.005 };
    assert!(
        elapsed.as_secs_f64() < max_allowed, // Sub-5ms in release, 50ms in debug
        "Evaluation took too long: {:?}",
        elapsed
    );
}

#[test]
fn test_multi_layer_collision_stack() {
    use libavicreator::GarmentLayer;

    // Base body: quad on z=0 plane
    let body_verts = vec![
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new( 1.0, -1.0, 0.0),
        Vec3::new( 1.0,  1.0, 0.0),
        Vec3::new(-1.0,  1.0, 0.0),
    ];
    let body_polys = vec![[0, 1, 2], [0, 2, 3]];
    let mut fitter = Fitter::new(body_verts, &body_polys);

    // Inner shirt (layer 1): parallel quad at z=0.10
    let shirt_layer = GarmentLayer {
        layer_depth: 1,
        vertices: vec![
            Vec3::new(-1.0, -1.0, 0.10),
            Vec3::new( 1.0, -1.0, 0.10),
            Vec3::new( 1.0,  1.0, 0.10),
            Vec3::new(-1.0,  1.0, 0.10),
        ],
        polygons: vec![[0, 1, 2], [0, 2, 3]],
    };
    fitter.add_inner_layer(shirt_layer);

    // Outer jacket (layer 2): asset vertex at (0.0, 0.0, 0.05) penetrating inner shirt
    let jacket_verts = vec![Vec3::new(0.0, 0.0, 0.05)];
    let mut config = libavicreator::FittingConfig::default();
    config.min_clearance = 0.005;

    let start = Instant::now();
    let fitted = fitter.fit_asset_layer(&jacket_verts, &config, 2);
    let latency = start.elapsed();

    assert_eq!(fitted.len(), 1);
    // Outer jacket must clear the inner shirt (z >= 0.10 + min_clearance)
    assert!(
        fitted[0].z >= 0.105 - 1e-4,
        "Outer jacket vertex at z={} penetrated inner shirt at z=0.10",
        fitted[0].z
    );

    // Clearance calculation latency under 50ms per layer
    assert!(latency.as_millis() < 50, "Latency {:?} exceeded 50ms", latency);
}

#[test]
fn test_fitter_and_rigging() {
    let body_verts = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    let body_polys = vec![[0, 1, 2]];
    let fitter = Fitter::new(body_verts, &body_polys);

    let asset_verts = vec![Vec3::new(0.2, 0.2, 0.05)];
    let config = Default::default();
    let fitted = fitter.fit_asset(&asset_verts, &config);
    assert_eq!(fitted.len(), 1);

    // Test Skeleton skinning
    let bone = Bone::new("root", Vec3::ZERO, Vec3::Y, None);
    let skeleton = Skeleton::new(vec![bone]);
    let poses = HashMap::new();
    let pose_mats = skeleton.compute_pose_matrices(&poses);
    let weights = vec![vec![VertexWeight {
        bone_index: 0,
        weight: 1.0,
    }]];
    let skinned = skeleton.skin_vertices(&[Vec3::new(0.0, 0.5, 0.0)], &weights, &pose_mats);
    assert_eq!(skinned.len(), 1);
}
