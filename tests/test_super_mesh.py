"""Tests for Multi-Archetype Super-Mesh Topology Architecture."""

import os
import sys
import time
from unittest.mock import MagicMock

# Mock Blender modules if running in standalone Python environment
class DummyObject:
    pass

class DummyOperator:
    pass

class DummyPanel:
    pass

class DummyMenu:
    pass

class DummyHeader:
    pass

class DummyPropertyGroup:
    pass

class DummyImportHelper:
    pass

class DummyExportHelper:
    pass

bpy_mock = MagicMock()
bpy_mock.app.version = (4, 2, 0)
bpy_mock.app.handlers.persistent = lambda fn: fn
bpy_mock.props._PropertyDeferred = type("PropertyDeferred", (), {})
bpy_mock.utils.register_classes_factory = lambda classes: (lambda: None, lambda: None)
bpy_mock.types.Object = DummyObject
bpy_mock.types.Operator = DummyOperator
bpy_mock.types.Panel = DummyPanel
bpy_mock.types.Menu = DummyMenu
bpy_mock.types.Header = DummyHeader
bpy_mock.types.PropertyGroup = DummyPropertyGroup
sys.modules["bpy"] = bpy_mock
sys.modules["bpy.app"] = bpy_mock.app
sys.modules["bpy.app.handlers"] = bpy_mock.app.handlers
sys.modules["bpy.types"] = bpy_mock.types
sys.modules["bpy.ops"] = bpy_mock.ops

if "rna_prop_ui" not in sys.modules:
    sys.modules["rna_prop_ui"] = MagicMock()

if "bmesh" not in sys.modules:
    sys.modules["bmesh"] = MagicMock()

if "idprop" not in sys.modules:
    sys.modules["idprop"] = MagicMock()

if "bpy_extras" not in sys.modules:
    bpy_extras_mock = MagicMock()
    bpy_extras_mock.io_utils.ImportHelper = DummyImportHelper
    bpy_extras_mock.io_utils.ExportHelper = DummyExportHelper
    sys.modules["bpy_extras"] = bpy_extras_mock
    sys.modules["bpy_extras.io_utils"] = bpy_extras_mock.io_utils
    sys.modules["bpy_extras.wm_utils"] = bpy_extras_mock.wm_utils
    sys.modules["bpy_extras.wm_utils.progress_report"] = bpy_extras_mock.wm_utils.progress_report

if "mathutils" not in sys.modules:
    mathutils_mock = MagicMock()
    class Vector(tuple):
        def __new__(cls, val=(0.0, 0.0, 0.0)):
            if isinstance(val, (int, float)):
                return super().__new__(cls, (float(val), float(val), float(val)))
            return super().__new__(cls, tuple(float(x) for x in val))

        def __add__(self, other):
            return Vector((self[0] + other[0], self[1] + other[1], self[2] + other[2]))

        def __sub__(self, other):
            return Vector((self[0] - other[0], self[1] - other[1], self[2] - other[2]))

        def __truediv__(self, other):
            return Vector((self[0] / float(other), self[1] / float(other), self[2] / float(other)))

    mathutils_mock.Vector = Vector
    sys.modules["mathutils"] = mathutils_mock

import numpy as np
import pytest

from lib import xml_base_mesh
from lib.morpher_cores import NumpyMorpher, MorpherCore
from lib.morphs import FullMorph, PartialMorph, MinMaxMorph
from lib import rigging


BASE_MESH_DIR = os.path.join(os.path.dirname(__file__), "..", "data", "base_meshes")
if not os.path.exists(os.path.join(BASE_MESH_DIR, "SuperMesh.xml")):
    BASE_MESH_DIR = os.path.join(os.path.dirname(__file__), "..", "base_meshes")
SUPER_MESH_XML = os.path.join(BASE_MESH_DIR, "SuperMesh.xml")


class MockCharacter:
    """Mock character object for testing NumpyMorpher without Blender dependency."""
    def __init__(self, np_basis, xml_mesh=None):
        self.np_basis = np_basis
        self.xml_base_mesh = xml_mesh
        self.custom_morph_order = False
        self.types = {}
        self.assets = {}
        self.path = lambda *args: os.path.join("/tmp", *args)


class MockVertices(list):
    def foreach_get(self, attr, arr):
        arr.fill(0.0)


class MockObjectData:
    """Mock Blender Object Data."""
    def __init__(self, vertex_count):
        self.vertices = MockVertices([None] * vertex_count)
        self.shape_keys = None
        self.store = {}

    def get(self, key, default=None):
        return self.store.get(key, default)

    def __getitem__(self, key):
        return self.store[key]

    def __setitem__(self, key, value):
        self.store[key] = value

    def update(self):
        pass


class MockTarget:
    """Mock Blender Mesh Target."""
    def foreach_set(self, attr, arr):
        pass


class MockObject(DummyObject):
    """Mock Blender Object."""
    def __init__(self, vertex_count):
        self.name = "MockSuperMeshObject"
        self.data = MockObjectData(vertex_count)
        self.store = {}

    def get(self, key, default=None):
        return self.store.get(key, default)


def mock_get_target(obj):
    return MockTarget()


def test_super_mesh_xml_parsing():
    """Requirement 1: Unified super-mesh XML files parse correctly into BaseMesh objects."""
    assert os.path.isfile(SUPER_MESH_XML), f"SuperMesh.xml not found at {SUPER_MESH_XML}"
    mesh = xml_base_mesh.load_base_mesh(SUPER_MESH_XML)

    assert isinstance(mesh, xml_base_mesh.BaseMesh)
    assert mesh.name == "SuperMesh"
    assert mesh.is_super_mesh is True
    assert len(mesh.vertices) == 21
    assert len(mesh.faces) == 10
    assert "tail.01" in mesh.bones
    assert "ear.01.L" in mesh.bones
    assert "ear.01.R" in mesh.bones

    # Check preallocated geometry
    assert "muzzle" in mesh.preallocated_geometry
    assert "tail" in mesh.preallocated_geometry
    assert "ears" in mesh.preallocated_geometry
    assert mesh.preallocated_geometry["muzzle"] == [8, 9, 10]
    assert mesh.preallocated_geometry["tail"] == [15, 16, 17, 18]

    # Check limb chains
    assert "tail" in mesh.limb_chains
    assert mesh.limb_chains["tail"] == ["tail.01", "tail.02", "tail.03", "tail.04"]

    # Test load_dir directory parser
    catalog = xml_base_mesh.load_dir(BASE_MESH_DIR)
    assert "SuperMesh" in catalog
    assert "HumanoidNeutral" in catalog
    assert "HumanoidAthletic" in catalog


def test_static_array_sizes_and_evaluations(monkeypatch):
    """Requirement 3: Morph core maintains static array sizes for super-mesh buffers."""
    monkeypatch.setattr("lib.utils.get_target", mock_get_target)

    # 21 vertices super-mesh basis
    basis = np.zeros((21, 3), dtype=np.float64)
    char = MockCharacter(np_basis=basis)
    obj = MockObject(21)

    morpher = NumpyMorpher(obj)
    morpher.char = char

    # Add non-humanoid morphs: muzzle and tail expansion
    muzzle_delta = np.zeros((21, 3), dtype=np.float64)
    muzzle_delta[8:11] = [0.0, 0.2, 0.0]  # Expand muzzle forward in Y
    muzzle_morph = MinMaxMorph("muzzle", [FullMorph(muzzle_delta)])

    tail_delta = np.zeros((21, 3), dtype=np.float64)
    tail_delta[15:19] = [0.0, -0.3, -0.1]  # Expand tail backward
    tail_morph = MinMaxMorph("tail", [FullMorph(tail_delta)])

    morpher.morphs_l2 = [muzzle_morph, tail_morph]

    # Evaluate with sliders = 0
    morpher.prop_set("muzzle", 0.0)
    morpher.prop_set("tail", 0.0)
    morpher.update()

    res0 = morpher.get_final()
    assert res0.shape == (21, 3)
    assert np.allclose(res0, 0.0)

    # Evaluate with muzzle slider = 1.0
    morpher.prop_set("muzzle", 1.0)
    morpher.update()

    res1 = morpher.get_final()
    assert res1.shape == (21, 3)
    assert np.allclose(res1[8:11], [0.0, 0.2, 0.0])
    # Non-muzzle vertices remain zero (collapsed/unmodified)
    assert np.allclose(res1[0:8], 0.0)


def test_unused_vertices_collapse_when_sliders_zero(monkeypatch):
    """Requirement 2: Unused non-humanoid vertices collapse cleanly to zero volume when sliders equal zero."""
    monkeypatch.setattr("lib.utils.get_target", mock_get_target)

    basis = np.zeros((21, 3), dtype=np.float64)
    char = MockCharacter(np_basis=basis)
    obj = MockObject(21)

    morpher = NumpyMorpher(obj)
    morpher.char = char

    # Create collapse delta for non-humanoid vertices
    ears_delta = np.zeros((21, 3), dtype=np.float64)
    ears_delta[11:15] = [0.15, 0.0, 0.2]  # Ear expansion delta
    ears_morph = MinMaxMorph("ears", [FullMorph(ears_delta)])

    morpher.morphs_l2 = [ears_morph]

    # Slider set to zero
    morpher.prop_set("ears", 0.0)
    morpher.update()

    final_mesh = morpher.get_final()
    # Vertices 11..14 remain at origin (collapsed zero volume)
    assert np.allclose(final_mesh[11:15], 0.0)


def test_non_humanoid_geometry_expansion(monkeypatch):
    """Acceptance Criteria 3: Non-humanoid morph sliders expand muzzle, tail, and leg geometry without mesh reconstruction."""
    monkeypatch.setattr("lib.utils.get_target", mock_get_target)

    basis = np.zeros((21, 3), dtype=np.float64)
    char = MockCharacter(np_basis=basis)
    obj = MockObject(21)

    morpher = NumpyMorpher(obj)
    morpher.char = char

    feline_delta = np.zeros((21, 3), dtype=np.float64)
    # Muzzle expansion
    feline_delta[8:11] = [0.0, 0.25, 0.0]
    # Tail expansion
    feline_delta[15:19] = [0.0, -0.4, -0.2]

    feline_morph = MinMaxMorph("feline", [FullMorph(feline_delta)])
    morpher.morphs_l2 = [feline_morph]

    # Set feline slider to 1.0
    morpher.prop_set("feline", 1.0)
    morpher.update()

    expanded = morpher.get_final()
    assert expanded.shape == (21, 3)  # Vertex count remains static at 21
    assert np.allclose(expanded[8:11], [0.0, 0.25, 0.0])
    assert np.allclose(expanded[15:19], [0.0, -0.4, -0.2])


def test_rig_generator_supermesh_bone_chains():
    """Requirement 4: Bone definitions in lib/rigging.py include pre-defined tail and ear bone chains."""
    assert hasattr(rigging, "DEFAULT_SUPERMESH_BONE_CHAINS")
    chains = rigging.DEFAULT_SUPERMESH_BONE_CHAINS

    assert "tail" in chains
    assert chains["tail"] == ["tail.01", "tail.02", "tail.03", "tail.04"]
    assert "ear_L" in chains
    assert chains["ear_L"] == ["ear.01.L", "ear.02.L"]
    assert "ear_R" in chains
    assert chains["ear_R"] == ["ear.01.R", "ear.02.R"]

    # Check default bone schema positions
    assert "tail.01" in rigging.DEFAULT_SUPERMESH_BONES
    assert "ear.01.L" in rigging.DEFAULT_SUPERMESH_BONES
    assert rigging.DEFAULT_SUPERMESH_BONES["tail.01"]["parent"] == "pelvis"
    assert rigging.DEFAULT_SUPERMESH_BONES["ear.01.L"]["parent"] == "head"

    # Test Rigger joint position fallback
    rigger = rigging.Rigger(None)
    class MockBone:
        def __init__(self, name):
            self.name = name
            self.parent = None
        def get(self, key, default=None):
            return default
    tail_bone = MockBone("tail.01")
    pos_head = rigger.joint_position(tail_bone, "head")
    pos_tail = rigger.joint_position(tail_bone, "tail")

    assert pos_head is not None
    assert pos_tail is not None
    assert pos_head[0] == 0.0 and pos_head[1] == -0.1 and pos_head[2] == 0.9


def test_morph_slider_performance_benchmark(monkeypatch):
    """Acceptance Criteria 5: Morph slider performance meets existing speed benchmarks across all character archetypes."""
    monkeypatch.setattr("lib.utils.get_target", mock_get_target)

    basis = np.random.randn(5000, 3)
    char = MockCharacter(np_basis=basis)
    obj = MockObject(5000)

    morpher = NumpyMorpher(obj)
    morpher.char = char

    # Generate 10 active morphs
    morph_list = []
    for i in range(10):
        delta = np.random.randn(5000, 3) * 0.01
        morph_list.append(MinMaxMorph(f"morph_{i}", [FullMorph(delta)]))

    morpher.morphs_l2 = morph_list

    t0 = time.perf_counter()
    iterations = 1000
    for i in range(iterations):
        val = (i % 100) / 100.0
        morpher.prop_set("morph_0", val)
        morpher.update()

    elapsed = time.perf_counter() - t0
    # 1000 evaluations should complete in less than 0.5 seconds on 5000-vertex super-mesh
    assert elapsed < 0.5, f"Performance benchmark failed: {iterations} iterations took {elapsed:.4f}s"
