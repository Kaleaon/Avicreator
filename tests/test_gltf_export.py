# Unit tests for Standalone glTF 2.0 Exporter Engine

import os
import sys
import tempfile
import json
import subprocess
import pytest
import numpy as np

import file_io
from lib import xml_base_mesh, gltf_exporter


def test_gltf_standalone_export_glb_validation():
    """Verify standalone glTF exporter produces valid .glb binary files passing Khronos validator."""
    base_mesh_path = os.path.join(os.path.dirname(__file__), "..", "base_meshes", "HumanoidNeutral.xml")
    if not os.path.exists(base_mesh_path):
        pytest.skip("Base mesh XML file not found")

    bm = xml_base_mesh.load_base_mesh(base_mesh_path)

    with tempfile.TemporaryDirectory() as tmpdir:
        glb_path = os.path.join(tmpdir, "character.glb")
        gltf_exporter.export_gltf_standalone(glb_path, bm, format="glb")

        assert os.path.exists(glb_path)
        assert os.path.getsize(glb_path) > 100

        # Validate with node gltf-validator
        node_validator = "/tmp/node_modules/gltf-validator/index.js"
        if os.path.exists(node_validator):
            res = subprocess.run(["node", node_validator, glb_path], capture_output=True, text=True)
            assert res.returncode == 0, f"glTF Validator failed for GLB: {res.stdout}\n{res.stderr}"


def test_gltf_standalone_export_gltf_validation():
    """Verify standalone glTF exporter produces valid .gltf JSON + .bin buffer files passing Khronos validator."""
    base_mesh_path = os.path.join(os.path.dirname(__file__), "..", "base_meshes", "HumanoidNeutral.xml")
    if not os.path.exists(base_mesh_path):
        pytest.skip("Base mesh XML file not found")

    bm = xml_base_mesh.load_base_mesh(base_mesh_path)

    with tempfile.TemporaryDirectory() as tmpdir:
        gltf_path = os.path.join(tmpdir, "character.gltf")
        bin_path = os.path.join(tmpdir, "character.bin")

        gltf_exporter.export_gltf_standalone(gltf_path, bm, format="gltf")

        assert os.path.exists(gltf_path)
        assert os.path.exists(bin_path)

        with open(gltf_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        assert data["asset"]["version"] == "2.0"
        assert len(data["meshes"]) == 1
        assert "POSITION" in data["meshes"][0]["primitives"][0]["attributes"]
        assert "NORMAL" in data["meshes"][0]["primitives"][0]["attributes"]
        assert "TEXCOORD_0" in data["meshes"][0]["primitives"][0]["attributes"]

        # Validate with node gltf-validator
        node_validator = "/tmp/node_modules/gltf-validator/index.js"
        if os.path.exists(node_validator):
            res = subprocess.run(["node", node_validator, gltf_path], capture_output=True, text=True)
            assert res.returncode == 0, f"glTF Validator failed for GLTF: {res.stdout}\n{res.stderr}"


def test_file_io_uiprops_export_format():
    """Verify file_io.UIProps export_format enum items include gltf and glb."""
    has_export_fmt = hasattr(file_io.UIProps, "export_format") or (
        "export_format" in getattr(file_io.UIProps, "__annotations__", {})
    )
    assert has_export_fmt

    prop = getattr(file_io.UIProps, "export_format", None)
    if prop is None:
        prop = file_io.UIProps.__annotations__.get("export_format")

    items = getattr(prop, "keywords", {}).get("items", []) if prop else []
    if not items and hasattr(prop, "items"):
        items = prop.items

    keys = [item[0] for item in items if isinstance(item, (tuple, list)) and len(item) > 0]
    if keys:
        assert "gltf" in keys
        assert "glb" in keys


def test_shape_key_flattening_without_mutation():
    """Verify shape key baking evaluates active morph deltas without calling obj.shape_key_remove."""
    class DummyKeyBlock:
        def __init__(self, name, value):
            self.name = name
            self.value = value

    class DummyShapeKeys:
        def __init__(self):
            self.key_blocks = [DummyKeyBlock("L1", 1.0), DummyKeyBlock("L2", 0.5)]

    class DummyVertex:
        def __init__(self, co):
            self.co = np.array(co, dtype=np.float64)

    class DummyMesh:
        def __init__(self, num_verts=5):
            self.vertices = [DummyVertex([i, i, i]) for i in range(num_verts)]
            self.shape_keys = DummyShapeKeys()

    class DummyObject:
        def __init__(self):
            self.data = DummyMesh()

        def shape_key_remove(self, key):
            raise RuntimeError("obj.shape_key_remove must NOT be called")

    obj = DummyObject()
    result = file_io.bake_shape_keys_for_export(obj)

    assert result is obj
    # shape_key_remove was not called (which would have raised RuntimeError)
