import mathutils
import numpy as np
import pytest
from lib import sliding_joints


class DummyArmature:
    def __init__(self, sliding_joints_dict):
        self.sliding_joints = sliding_joints_dict


def create_dummy_char():
    arm = DummyArmature({
        "knee": {
            "calc": "verts_knee * 0.5 + vol_vert_0 * 0.1",
            "verts_knee": [(0, 1)],
            "vol_vert_0": 0,
            "default_influence": 0.1,
        }
    })
    class Char:
        armature = {"test_rig": arm}
    return Char()


def test_sjcalc_post_relaxation_volumetric_metrics():
    """Verify SJCalc receives post-relaxation surface volumetric metrics and updates joint influence."""

    def mock_get_co(idx):
        coords = {0: mathutils.Vector([0.0, 1.0, 0.0]), 1: mathutils.Vector([0.0, 2.0, 0.0])}
        return coords.get(idx, mathutils.Vector([0.0, 0.0, 0.0]))

    dummy_char = create_dummy_char()

    sj_calc = sliding_joints.SJCalc(dummy_char, rig=None, get_co=mock_get_co)

    # Set post-relaxation volumetric metrics
    vol_center = mathutils.Vector([0.0, 0.5, 0.0])
    normals = np.array([[0.0, 1.0, 0.0], [0.0, 1.0, 0.0]])
    sj_calc.update_volumetric_metrics(
        volumetric_center=vol_center,
        mesh_volume=12.5,
        surface_normals=normals,
    )

    assert sj_calc.mesh_volume == 12.5
    np.testing.assert_allclose(sj_calc.volumetric_center, vol_center)
    assert sj_calc.sample_surface_normal(0) is not None

    # Calculate volumetric distance from vol_center (0, 0.5, 0) to vertex 0 (0, 1.0, 0) -> dist = 0.5
    vol_dist = sj_calc.calc_volumetric_distance(0)
    assert abs(vol_dist - 0.5) < 1e-5

    # Recalculate influence: verts_knee = dist(0, 1) = 1.0, vol_vert_0 = 0.5
    # calc: 1.0 * 0.5 + 0.5 * 0.1 = 0.55
    sj_calc.recalc()
    inf = sj_calc.influence.get("test_rig", {}).get("knee")
    assert inf is not None
    assert abs(inf - 0.55) < 1e-4
