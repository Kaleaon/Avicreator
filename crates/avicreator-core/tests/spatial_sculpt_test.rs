use avicreator_core::{
    BVHTree, FalloffCurve, Mesh, Ray, SculptContext, SculptMode, Vec3,
};
use std::time::Instant;

#[test]
fn test_bvh_construction_and_query_performance() {
    // Create a 100x100 grid mesh (10,000 vertices, ~19,600 triangles)
    let size = 100;
    let mut positions = Vec::with_capacity(size * size);
    let mut indices = Vec::with_capacity((size - 1) * (size - 1) * 6);

    for y in 0..size {
        for x in 0..size {
            positions.push(Vec3::new(x as f32 * 0.1, y as f32 * 0.1, 0.0));
        }
    }

    for y in 0..(size - 1) {
        for x in 0..(size - 1) {
            let i0 = (y * size + x) as u32;
            let i1 = (y * size + x + 1) as u32;
            let i2 = ((y + 1) * size + x) as u32;
            let i3 = ((y + 1) * size + x + 1) as u32;

            indices.extend_from_slice(&[i0, i1, i2]);
            indices.extend_from_slice(&[i1, i3, i2]);
        }
    }

    let mut mesh = Mesh::new(positions, indices);
    mesh.recalculate_normals();

    let start_build = Instant::now();
    let bvh = BVHTree::from_mesh(&mesh);
    let build_duration = start_build.elapsed();
    println!("BVH build time for {} tris: {:?}", bvh.num_triangles, build_duration);

    // Query radius performance test
    let query_center = Vec3::new(5.0, 5.0, 0.0);
    let radius = 1.0;
    let mut visited_tris = Vec::new();
    let mut visited_verts = Vec::new();
    let mut vert_flags = vec![false; mesh.positions.len()];

    let start_query = Instant::now();
    bvh.query_radius(query_center, radius, &mesh.positions, &mut visited_tris, &mut visited_verts, &mut vert_flags);
    let query_duration = start_query.elapsed();

    println!("BVH radius query time: {:?}", query_duration);
    assert!(
        query_duration.as_millis() <= 2,
        "BVH radius query took {:?}, expected <= 2ms",
        query_duration
    );
    assert!(!visited_verts.is_empty(), "Should find candidate vertices within radius");
}

#[test]
fn test_raycast_mesh_hit() {
    let positions = vec![
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    let indices = vec![0, 1, 2];
    let mesh = Mesh::new(positions, indices);
    let bvh = BVHTree::from_mesh(&mesh);

    let origin = Vec3::new(0.0, 0.0, 5.0);
    let direction = Vec3::new(0.0, 0.0, -1.0);

    let hit = bvh.ray_cast(origin, direction, 10.0).expect("Ray should hit triangle");
    assert!((hit.point.x - 0.0).abs() < 1e-4);
    assert!((hit.point.y - 0.0).abs() < 1e-4);
    assert!((hit.point.z - 0.0).abs() < 1e-4);
    assert_eq!(hit.face_index, 0);
    assert!((hit.distance - 5.0).abs() < 1e-4);
}

#[test]
fn test_falloff_curves() {
    let radius = 2.0;

    // Smoothstep: t=0 -> 1.0, t=1.0 -> 0.0
    assert!((FalloffCurve::Smoothstep.evaluate(0.0, radius) - 1.0).abs() < 1e-5);
    assert!((FalloffCurve::Smoothstep.evaluate(2.0, radius) - 0.0).abs() < 1e-5);
    assert!((FalloffCurve::Smoothstep.evaluate(1.0, radius) - 0.5).abs() < 1e-5);

    // Gaussian: t=0 -> 1.0, t=1.0 -> 0.0
    assert!((FalloffCurve::Gaussian.evaluate(0.0, radius) - 1.0).abs() < 1e-5);
    assert!((FalloffCurve::Gaussian.evaluate(2.0, radius) - 0.0).abs() < 1e-5);
    assert!(FalloffCurve::Gaussian.evaluate(1.0, radius) > 0.0);
}

#[test]
fn test_sculpt_push_pull_smooth_and_preserve_attributes() {
    let size = 10;
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut bone_weights = Vec::new();
    let mut bone_indices = Vec::new();
    let mut indices = Vec::new();

    for y in 0..size {
        for x in 0..size {
            positions.push(Vec3::new(x as f32, y as f32, 0.0));
            uvs.push([x as f32 / 10.0, y as f32 / 10.0]);
            bone_weights.push([0.7, 0.3, 0.0, 0.0]);
            bone_indices.push([0, 1, 0, 0]);
        }
    }

    for y in 0..(size - 1) {
        for x in 0..(size - 1) {
            let i0 = (y * size + x) as u32;
            let i1 = (y * size + x + 1) as u32;
            let i2 = ((y + 1) * size + x) as u32;
            let i3 = ((y + 1) * size + x + 1) as u32;
            indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]);
        }
    }

    let mut mesh = Mesh {
        positions,
        normals: vec![Vec3::Z; size * size],
        uvs,
        indices,
        bone_weights,
        bone_indices,
    };
    mesh.recalculate_normals();

    let initial_uvs = mesh.uvs.clone();
    let initial_bone_weights = mesh.bone_weights.clone();
    let initial_bone_indices = mesh.bone_indices.clone();

    let bvh = BVHTree::from_mesh(&mesh);
    let mut ctx = SculptContext::new();

    // Push brush action
    let center = Vec3::new(5.0, 5.0, 0.0);
    ctx.apply_stroke(
        &mut mesh,
        &bvh,
        center,
        2.0,
        1.5,
        SculptMode::Push,
        FalloffCurve::Gaussian,
        Vec3::Z,
    );

    // Verify center vertex was displaced along +Z
    let center_idx = 5 * size + 5;
    assert!(mesh.positions[center_idx].z > 1.0, "Center vertex should be pushed outward");

    // Verify UVs and bone bindings were preserved
    assert_eq!(mesh.uvs, initial_uvs, "UV coordinates must be preserved");
    assert_eq!(mesh.bone_weights, initial_bone_weights, "Bone weights must be preserved");
    assert_eq!(mesh.bone_indices, initial_bone_indices, "Bone indices must be preserved");

    // Verify normals updated
    assert!(mesh.normals[center_idx].length_squared() > 0.9, "Normal vectors must be normalized");

    // Pull brush action
    let updated_bvh = BVHTree::from_mesh(&mesh);
    ctx.apply_stroke(
        &mut mesh,
        &updated_bvh,
        center,
        2.0,
        1.0,
        SculptMode::Pull,
        FalloffCurve::Smoothstep,
        Vec3::Z,
    );

    // Smooth brush action
    let updated_bvh2 = BVHTree::from_mesh(&mesh);
    ctx.apply_stroke(
        &mut mesh,
        &updated_bvh2,
        center,
        2.0,
        0.5,
        SculptMode::Smooth,
        FalloffCurve::Gaussian,
        Vec3::ZERO,
    );
}
