import sys
import types
import time
import numpy as np

import pytest
from lib.fit_calc import Geometry, SoftBinder, HardBinder
from lib.fitting import apply_surface_clearance_and_relaxation, build_adjacency_list


def create_cube_geometry(scale=1.0, offset=(0.0, 0.0, 0.0)):
    half = 0.5 * scale
    ox, oy, oz = offset
    verts = np.array([
        [-half + ox, -half + oy, -half + oz], # 0
        [ half + ox, -half + oy, -half + oz], # 1
        [ half + ox,  half + oy, -half + oz], # 2
        [-half + ox,  half + oy, -half + oz], # 3
        [-half + ox, -half + oy,  half + oz], # 4
        [ half + ox, -half + oy,  half + oz], # 5
        [ half + ox,  half + oy,  half + oz], # 6
        [-half + ox,  half + oy,  half + oz], # 7
    ], dtype=np.float64)

    faces = [
        (0, 3, 2, 1),  # bottom (-Z)
        (4, 5, 6, 7),  # top (+Z)
        (0, 1, 5, 4),  # front (-Y)
        (2, 3, 7, 6),  # back (+Y)
        (0, 4, 7, 3),  # left (-X)
        (1, 2, 6, 5),  # right (+X)
    ]
    return Geometry(verts, faces)


def test_geometry_normals_and_adaptive_threshold():
    geom_small = create_cube_geometry(scale=1.0)
    geom_large = create_cube_geometry(scale=10.0)

    assert geom_small.bbox_scale < geom_large.bbox_scale

    thresh_small = geom_small.get_adaptive_dist_thresh()
    thresh_large = geom_large.get_adaptive_dist_thresh()
    assert thresh_large > thresh_small

    normals = geom_small.vertex_normals
    assert normals.shape == geom_small.verts.shape
    norms = np.linalg.norm(normals, axis=1)
    np.testing.assert_allclose(norms, 1.0, rtol=1e-5)


def test_normal_gated_binding():
    char_geom = create_cube_geometry(scale=2.0)

    asset_verts_geom1 = np.array([
        [0.0, 0.0, 1.1],
        [0.1, 0.0, 1.1],
        [0.0, 0.1, 1.1],
    ], dtype=np.float64)
    asset_geom_aligned = Geometry(asset_verts_geom1, [(0, 1, 2)])

    asset_verts_geom2 = np.array([
        [0.0, 0.0, 1.1],
        [0.0, 0.1, 1.1],
        [0.1, 0.0, 1.1],
    ], dtype=np.float64)
    asset_geom_opposed = Geometry(asset_verts_geom2, [(0, 1, 2)])

    binder_aligned = SoftBinder(char_geom, asset_verts_geom1[:1], asset_geom_aligned)
    binder_aligned.calc_binding_kd()

    binder_opposed = SoftBinder(char_geom, asset_verts_geom2[:1], asset_geom_opposed)
    binder_opposed.calc_binding_kd()

    weight_aligned = sum(binder_aligned.bindings[0].values()) if binder_aligned.bindings else 0
    weight_opposed = sum(binder_opposed.bindings[0].values()) if binder_opposed.bindings else 0

    assert weight_aligned > weight_opposed


def test_post_fitting_surface_clearance_projection():
    char_geom = create_cube_geometry(scale=2.0)

    asset_verts = np.array([
        [0.0, 0.0, 0.95],  # Penetrating body by 0.05
        [0.0, 0.0, 1.05],  # Safe clearance
    ], dtype=np.float64)
    asset_faces = [(0, 1)]

    min_clearance = 0.002
    result = apply_surface_clearance_and_relaxation(
        asset_verts, asset_faces, char_geom,
        min_clearance=min_clearance,
        max_relaxation_passes=0
    )

    assert result[0][2] >= 1.0 + min_clearance - 1e-6
    assert result[1][2] == pytest.approx(1.05)


def test_masked_body_vertex_exclusion():
    char_geom = create_cube_geometry(scale=2.0)
    # Top face vertices in create_cube_geometry: (4, 5, 6, 7)
    masked_verts = {4, 5, 6, 7}

    asset_verts = np.array([
        [0.0, 0.0, 0.95],
    ], dtype=np.float64)
    asset_faces = [(0, 0)]

    min_clearance = 0.002
    result = apply_surface_clearance_and_relaxation(
        asset_verts, asset_faces, char_geom,
        masked_body_verts=masked_verts,
        min_clearance=min_clearance,
        max_relaxation_passes=0
    )

    assert result[0][2] == pytest.approx(0.95)


def test_laplacian_relaxation_smoothing_and_cap():
    char_geom = create_cube_geometry(scale=2.0)

    asset_verts = np.array([
        [0.0, 0.0, 1.005],
        [0.1, 0.0, 1.005],
        [0.2, 0.0, 1.500],  # Spike
        [0.3, 0.0, 1.005],
        [0.4, 0.0, 1.005],
    ], dtype=np.float64)

    asset_faces = [(0, 1), (1, 2), (2, 3), (3, 4)]

    start_time = time.time()
    result = apply_surface_clearance_and_relaxation(
        asset_verts, asset_faces, char_geom,
        min_clearance=0.002,
        max_relaxation_passes=10
    )
    elapsed_ms = (time.time() - start_time) * 1000

    assert result[2][2] < 1.500

    for v in result:
        assert v[2] >= 1.0 + 0.002 - 1e-6

    assert elapsed_ms < 120.0
