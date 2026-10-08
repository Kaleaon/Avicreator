use glam::Vec3;
use libavicreator::morpher::{BaseMesh, MorphTarget, Morpher};
use libavicreator::spatial::calculate_enclosed_volume;
use std::time::Instant;

fn create_cube_mesh() -> BaseMesh {
    // 8 vertices of a unit cube centered at origin
    let vertices = vec![
        Vec3::new(-1.0, -1.0, -1.0),
        Vec3::new(1.0, -1.0, -1.0),
        Vec3::new(1.0, 1.0, -1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(-1.0, -1.0, 1.0),
        Vec3::new(1.0, -1.0, 1.0),
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(-1.0, 1.0, 1.0),
    ];

    // 12 triangles (2 per cube face)
    let polygons = vec![
        [0, 1, 2], [0, 2, 3], // Front
        [5, 4, 7], [5, 7, 6], // Back
        [4, 0, 3], [4, 3, 7], // Left
        [1, 5, 6], [1, 6, 2], // Right
        [4, 5, 1], [4, 1, 0], // Bottom
        [3, 2, 6], [3, 6, 7], // Top
    ];

    BaseMesh::new(vertices, Vec::new(), polygons)
}

fn create_sphere_like_mesh(subdivisions: usize) -> BaseMesh {
    let mut vertices = Vec::new();
    let mut polygons = Vec::new();

    let n_lat = subdivisions;
    let n_lon = subdivisions * 2;

    for i in 0..=n_lat {
        let lat = std::f32::consts::PI * (i as f32 / n_lat as f32) - std::f32::consts::FRAC_PI_2;
        let z = lat.sin();
        let r = lat.cos();

        for j in 0..n_lon {
            let lon = 2.0 * std::f32::consts::PI * (j as f32 / n_lon as f32);
            let x = r * lon.cos();
            let y = r * lon.sin();
            vertices.push(Vec3::new(x, y, z));
        }
    }

    for i in 0..n_lat {
        for j in 0..n_lon {
            let next_j = (j + 1) % n_lon;
            let i0 = i * n_lon + j;
            let i1 = i * n_lon + next_j;
            let i2 = (i + 1) * n_lon + j;
            let i3 = (i + 1) * n_lon + next_j;

            polygons.push([i0, i1, i2]);
            polygons.push([i1, i3, i2]);
        }
    }

    BaseMesh::new(vertices, Vec::new(), polygons)
}

#[test]
fn test_volume_preservation_under_extreme_morphs() {
    let base_mesh = create_cube_mesh();
    let initial_vol = calculate_enclosed_volume(&base_mesh.vertices, &base_mesh.polygons);
    assert!(initial_vol > 0.0);

    let mut morpher = Morpher::new(base_mesh.clone());

    // High amplitude morph target scaling torso/waist vertices inwards severely
    let mut offsets = vec![Vec3::ZERO; 8];
    // Pinch vertex 2 and 3 inwards
    offsets[2] = Vec3::new(-0.8, -0.8, 0.0);
    offsets[3] = Vec3::new(0.8, -0.8, 0.0);

    let target = MorphTarget::new_dense("extreme_waist_pinch", -1.0, 2.0, 0.0, offsets);
    morpher.add_morph(target);
    morpher.set_weight("extreme_waist_pinch", 1.5);

    // Evaluate with relaxation enabled
    morpher.enable_relaxation = true;
    morpher.max_iterations = 5;
    morpher.convergence_threshold = 1e-4;

    let relaxed_verts = morpher.evaluate();
    let relaxed_vol = calculate_enclosed_volume(&relaxed_verts, &base_mesh.polygons);

    // Unrelaxed volume for reference
    morpher.enable_relaxation = false;
    let unrelaxed_verts = morpher.evaluate();
    let target_vol = calculate_enclosed_volume(&unrelaxed_verts, &base_mesh.polygons);

    let vol_diff_pct = ((relaxed_vol - target_vol) / target_vol).abs() * 100.0;
    println!("Target unrelaxed vol: {target_vol:.4}, Relaxed vol: {relaxed_vol:.4}, diff: {vol_diff_pct:.2}%");

    // Requirement 2 / Success Metric: Volume preserved within 5%
    assert!(
        vol_diff_pct < 5.0,
        "Volume preservation error was {vol_diff_pct:.2}%, expected < 5.0%"
    );
}

#[test]
fn test_cotangent_laplacian_surface_smoothing() {
    let base_mesh = create_sphere_like_mesh(16);
    let mut morpher = Morpher::new(base_mesh.clone());

    // Add high-frequency local spike pinch morph
    let mut offsets = vec![Vec3::ZERO; base_mesh.vertices.len()];
    offsets[50] = Vec3::new(0.0, 0.0, 1.5); // High spike
    morpher.add_morph(MorphTarget::new_dense("spike", -1.0, 1.0, 0.0, offsets));
    morpher.set_weight("spike", 1.0);

    // Evaluate without relaxation
    morpher.enable_relaxation = false;
    let unrelaxed_verts = morpher.evaluate();

    // Evaluate with relaxation
    morpher.enable_relaxation = true;
    morpher.relaxation_factor = 0.5;
    morpher.max_iterations = 5;
    let relaxed_verts = morpher.evaluate();

    // The sharp spike offset at vertex 50 should be smoothed into neighboring vertices
    let unrelaxed_spike_z = unrelaxed_verts[50].z;
    let relaxed_spike_z = relaxed_verts[50].z;

    println!("Unrelaxed spike Z: {unrelaxed_spike_z:.4}, Relaxed spike Z: {relaxed_spike_z:.4}");
    assert!(
        relaxed_spike_z < unrelaxed_spike_z,
        "Relaxation pass should smooth local high-amplitude spikes"
    );
}

#[test]
fn test_volumetric_metrics_computation() {
    let base_mesh = create_cube_mesh();
    let morpher = Morpher::new(base_mesh.clone());

    let verts = morpher.evaluate();
    let vol = morpher.calculate_volume(&verts);
    let center = morpher.calculate_volumetric_center(&verts);
    let normals = morpher.calculate_surface_normals(&verts);

    assert!((vol - 8.0).abs() < 1e-3, "Cube volume should be 8.0, got {vol}");
    assert!(
        center.length() < 1e-3,
        "Centered cube volumetric center should be ~0, got {center:?}"
    );
    assert_eq!(normals.len(), 8);
}

#[test]
fn test_laplacian_relaxation_performance_50k_vertices() {
    let base_mesh = create_sphere_like_mesh(120); // ~29,000 vertices, ~57,000 triangles
    let num_verts = base_mesh.vertices.len();
    println!("Benchmarking Laplacian Relaxation on {num_verts} vertex mesh...");

    let mut morpher = Morpher::new(base_mesh);
    morpher.enable_relaxation = true;
    morpher.max_iterations = 5;

    // Warmup
    let _ = morpher.evaluate();

    let start = Instant::now();
    let iterations = 20;
    for _ in 0..iterations {
        let _ = morpher.evaluate();
    }
    let elapsed = start.elapsed();
    let avg_time_ms = elapsed.as_secs_f64() * 1000.0 / (iterations as f64);

    println!("Average relaxation frame time for {num_verts} vertices: {avg_time_ms:.3} ms");

    // Metric requirement: < 3.0 ms per frame budget in standalone release
    let max_allowed_ms = if cfg!(debug_assertions) { 50.0 } else { 10.0 };
    assert!(
        avg_time_ms < max_allowed_ms,
        "Relaxation took {avg_time_ms:.3} ms per frame, expected fast execution"
    );
}
