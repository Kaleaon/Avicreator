import time
import pytest
import numpy as np
import libavicreator

def test_imports_and_version():
    """Verify libavicreator imports cleanly without Blender/bpy and returns version."""
    assert hasattr(libavicreator, "version")
    ver = libavicreator.version()
    assert isinstance(ver, str)
    assert ver == "0.1.0"

def test_bvh_ray_cast_and_nearest():
    """Verify BVHTree ray casting and nearest surface queries match geometric expectations."""
    # Unit triangle on z=0 plane
    vertices = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
    polygons = [[0, 1, 2]]

    bvh = libavicreator.BVHTree.FromPolygons(vertices, polygons)

    # Ray cast from (0.2, 0.2, 1.0) towards -Z
    hit = bvh.ray_cast([0.2, 0.2, 1.0], [0.0, 0.0, -1.0], 5.0)
    assert hit is not None
    hit_pt, hit_norm, face_idx, dist = hit

    np.testing.assert_allclose(hit_pt, [0.2, 0.2, 0.0], atol=1e-5)
    np.testing.assert_allclose(hit_norm, [0.0, 0.0, 1.0], atol=1e-5)
    assert face_idx == 0
    assert abs(dist - 1.0) < 1e-5

    # Find nearest from point (0.2, 0.2, 0.5)
    near = bvh.find_nearest([0.2, 0.2, 0.5], 2.0)
    assert near is not None
    near_pt, near_norm, near_idx, near_dist = near
    np.testing.assert_allclose(near_pt, [0.2, 0.2, 0.0], atol=1e-5)
    assert near_idx == 0
    assert abs(near_dist - 0.5) < 1e-5

def test_kdtree_spatial_indexing():
    """Verify KDTree nearest neighbor and range searches."""
    points = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 2.0, 0.0],
        [0.0, 0.0, 3.0],
    ]
    kdtree = libavicreator.KDTree.build(points)

    found = kdtree.find([0.05, 0.05, 0.0])
    assert found is not None
    pt, idx, dist = found
    assert idx == 0
    np.testing.assert_allclose(pt, [0.0, 0.0, 0.0], atol=1e-5)

    n_found = kdtree.find_n([0.0, 0.0, 0.0], 2)
    assert len(n_found) == 2
    assert n_found[0][1] == 0
    assert n_found[1][1] == 1

def test_morpher_accuracy_and_performance_benchmark():
    """Verify Morpher evaluation accuracy and sub-millisecond 50,000 vertex evaluation speed."""
    num_verts = 50_000
    base_verts = [[i * 0.001, (i % 100) * 0.001, 0.0] for i in range(num_verts)]
    polys = []

    morpher = libavicreator.Morpher(base_verts, polys)

    # Add 10 dense morph targets
    for m in range(10):
        offsets = [[0.001 * (m + 1), 0.0, 0.0001 * (i % 10)] for i in range(num_verts)]
        morpher.add_morph_dense(f"morph_{m}", -1.0, 1.0, 0.0, offsets)
        morpher.set_weight(f"morph_{m}", 0.5)

    # Warmup
    _ = morpher.evaluate()

    # Benchmark evaluation time over 50 iterations
    start_time = time.perf_counter()
    iterations = 50
    for _ in range(iterations):
        result = morpher.evaluate()
    total_time = time.perf_counter() - start_time
    avg_frame_time_ms = (total_time / iterations) * 1000.0

    print(f"\nAverage 50k vertex morph evaluation time: {avg_frame_time_ms:.3f} ms/frame")

    # Success Metric: Sub-millisecond evaluation speed per frame for 50,000 vertices
    assert avg_frame_time_ms < 1.0, f"Morph evaluation took {avg_frame_time_ms:.3f} ms, expected < 1.0 ms"

    # Numerical Parity check
    assert isinstance(result, np.ndarray)
    assert result.shape == (num_verts, 3)

    # Hand-calculate expected vertex 0 position
    # Base: [0.0, 0.0, 0.0] + sum_m(0.5 * [0.001 * (m+1), 0.0, 0.0])
    expected_x = sum(0.5 * 0.001 * (m + 1) for m in range(10))
    np.testing.assert_allclose(result[0, 0], expected_x, atol=1e-5)

def test_fitter_and_rigger():
    """Verify Fitter and Rigger native classes."""
    body_verts = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
    body_polys = [[0, 1, 2]]

    fitter = libavicreator.Fitter(body_verts, body_polys)
    asset_verts = [[0.2, 0.2, 0.1]]
    fitted = fitter.fit_asset(asset_verts, max_distance=0.5, smoothing_iterations=1)

    assert isinstance(fitted, np.ndarray)
    assert fitted.shape == (1, 3)

    rigger = libavicreator.Rigger()
    rigger.add_bone("root", [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], None)
    weights = [[(0, 1.0)]]
    skinned = rigger.skin_vertices([[0.0, 0.5, 0.0]], weights, None)

    assert isinstance(skinned, np.ndarray)
    assert skinned.shape == (1, 3)

def test_asset_manifest_and_skeleton_schema():
    """Verify AssetManifest and Skeleton PyO3 bindings and JSON roundtrip."""
    manifest = libavicreator.AssetManifest(
        id="hero_v1",
        name="Hero Character",
        version="1.0.0",
        category="character",
        skeleton_type="biped"
    )
    manifest.author = "Kaleaon Studio"
    manifest.mesh_files = ["body.obj", "head.obj"]
    manifest.morph_targets = ["smile", "blink"]
    manifest.tags = ["hero", "avatar"]

    assert manifest.id == "hero_v1"
    assert manifest.name == "Hero Character"
    assert manifest.category == "character"
    assert manifest.author == "Kaleaon Studio"
    assert manifest.mesh_files == ["body.obj", "head.obj"]

    json_str = manifest.to_json()
    assert '"id": "hero_v1"' in json_str or '"id":"hero_v1"' in json_str

    parsed_manifest = libavicreator.AssetManifest.from_json(json_str)
    assert parsed_manifest.id == manifest.id
    assert parsed_manifest.name == manifest.name
    assert parsed_manifest.category == manifest.category
    assert parsed_manifest.skeleton_type == manifest.skeleton_type

    skel = libavicreator.Skeleton("StandardBiped")
    skel.add_node(0, "Hips", translation=[0.0, 0.0, 0.0])
    skel.add_node(1, "Chest", parent_id=0, translation=[0.0, 0.5, 0.0])
    skel.add_node(2, "Head", parent_id=1, translation=[0.0, 0.4, 0.0])

    assert skel.name == "StandardBiped"
    assert skel.node_count == 3
    assert skel.root_indices == [0]

    skel_json = skel.to_json()
    assert '"StandardBiped"' in skel_json

    parsed_skel = libavicreator.Skeleton.from_json(skel_json)
    assert parsed_skel.name == skel.name
    assert parsed_skel.node_count == skel.node_count
    assert parsed_skel.root_indices == skel.root_indices

def test_json_base_mesh_loading(tmp_path):
    """Verify loading base mesh from JSON format alongside XML fallback."""
    from lib.xml_base_mesh import load_base_mesh, load_dir

    json_content = """{
        "name": "JsonHero",
        "version": "1.0",
        "metadata": {"author": "Kaleaon"},
        "vertices": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        "faces": [[0, 1, 2]],
        "nodes": [
            {"id": 0, "name": "Hips", "translation": [0.0, 0.0, 0.0]},
            {"id": 1, "name": "Spine", "parent_id": 0, "translation": [0.0, 0.2, 0.0]}
        ]
    }"""

    json_file = tmp_path / "JsonHero.json"
    json_file.write_text(json_content)

    mesh = load_base_mesh(str(json_file))
    assert mesh.name == "JsonHero"
    assert mesh.version == "1.0"
    assert len(mesh.vertices) == 3
    assert len(mesh.faces) == 1
    assert "Hips" in mesh.bones
    assert "Spine" in mesh.bones

    loaded_dir = load_dir(str(tmp_path))
    assert "JsonHero" in loaded_dir


