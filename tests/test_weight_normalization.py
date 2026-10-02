"""Unit tests for dual-stage weight layer normalization and slider clamping."""

import json
import tempfile
import time
from pathlib import Path
import numpy as np
import pytest

from webapp.processing import ModelIngestionPipeline, persist_upload
from lib.xml_base_mesh import WeightLayer, BaseMesh, load_base_mesh, load_dir
import finalize


def test_slider_maximum_bounds_clamped(tmp_path):
    """Confirm all SliderDefinition instances produced by ModelIngestionPipeline have maximum <= 1.0."""
    upload_dir = tmp_path / "upload"
    upload_dir.mkdir()
    sample_model = upload_dir / "test_model.obj"
    sample_model.write_text("# Test OBJ file\nv 0 0 0\nv 1 1 1\n", encoding="utf-8")

    out_dir = tmp_path / "output"
    pipeline = ModelIngestionPipeline(
        upload_root=upload_dir,
        base_mesh_id="HumanoidNeutral",
        dispose_source=False,
        output_root=out_dir,
    )
    report = pipeline.run()

    assert report.success is True
    assert len(report.layer_summaries) > 0

    for summary in report.layer_summaries:
        assert summary.normalised is True
        assert summary.max_weight <= 1.0
        for slider in summary.sliders:
            assert slider.maximum <= 1.0
            assert slider.default_value <= slider.maximum
            assert slider.minimum == 0.0


def test_slider_metadata_serialization(tmp_path):
    """Confirm slider_metadata.json contains cross_layer_normalization and normalised: true flags."""
    upload_dir = tmp_path / "upload"
    upload_dir.mkdir()
    sample_model = upload_dir / "test_model.obj"
    sample_model.write_text("# Test OBJ file\nv 0 0 0\n", encoding="utf-8")

    out_dir = tmp_path / "output"
    pipeline = ModelIngestionPipeline(
        upload_root=upload_dir,
        base_mesh_id="HumanoidNeutral",
        dispose_source=False,
        output_root=out_dir,
    )
    report = pipeline.run()

    metadata_file = Path(report.generated_assets["slider_metadata"])
    assert metadata_file.exists()

    data = json.loads(metadata_file.read_text(encoding="utf-8"))
    assert "cross_layer_normalization" in data
    assert data["cross_layer_normalization"]["enabled"] is True
    assert data["cross_layer_normalization"]["target_weight_sum"] == 1.0
    assert data["cross_layer_normalization"]["max_slider_bound"] == 1.0

    for layer in data["layers"]:
        assert layer["normalised"] is True
        for slider in layer["sliders"]:
            assert slider["max"] <= 1.0
            assert slider["default"] <= slider["max"]


def test_xml_weight_layer_integrity_and_normalized_arrays():
    """Confirm original XML weight integrity is maintained while exposing normalized evaluation arrays."""
    # Create a WeightLayer with unconstrained raw weights > 1.0
    raw_weights = {
        "BoneA": {0: 0.8, 1: 0.5},
        "BoneB": {0: 0.6, 1: 0.7},  # Vertex 0 total = 1.4, Vertex 1 total = 1.2
    }
    layer = WeightLayer(
        name="muscle",
        layer_type="muscle",
        normalised=False,
        weights=raw_weights,
    )

    # Raw weights dict must remain untouched
    assert layer.weights["BoneA"][0] == 0.8
    assert layer.weights["BoneB"][0] == 0.6

    # Dense numpy without normalization
    unnorm = layer.as_numpy(vertex_count=2, normalize=False)
    assert unnorm["BoneA"][0] == 0.8
    assert unnorm["BoneB"][0] == 0.6

    # Normalized numpy arrays
    norm = layer.as_normalized_numpy(vertex_count=2)
    assert norm["BoneA"][0] == pytest.approx(0.8 / 1.4)
    assert norm["BoneB"][0] == pytest.approx(0.6 / 1.4)

    sum_v0 = norm["BoneA"][0] + norm["BoneB"][0]
    sum_v1 = norm["BoneA"][1] + norm["BoneB"][1]

    assert sum_v0 == pytest.approx(1.0)
    assert sum_v1 == pytest.approx(1.0)

    # Test via BaseMesh helper
    mesh = BaseMesh(
        name="TestMesh",
        version="1.0",
        metadata={},
        vertices=[(0, 0, 0), (1, 1, 1)],
        faces=[],
        bones={},
        weight_layers={"muscle": layer},
        sizing={},
    )

    eval_arrays = mesh.get_normalized_layer_weights("muscle")
    assert eval_arrays is not None
    assert eval_arrays["BoneA"][0] + eval_arrays["BoneB"][0] == pytest.approx(1.0)

    # Verify original layer weights in mesh were not modified
    assert mesh.weight_layers["muscle"].weights["BoneA"][0] == 0.8


class MockVertexGroupElement:
    def __init__(self, group: int, weight: float):
        self.group = group
        self.weight = weight


class MockVertex:
    def __init__(self, groups):
        self.groups = groups


class MockVertexGroup:
    def __init__(self, index: int, name: str):
        self.index = index
        self.name = name


class MockMeshData:
    def __init__(self, vertices):
        self.vertices = vertices


class MockObject:
    def __init__(self, num_vertices, unconstrained=True):
        self.vertex_groups = [
            MockVertexGroup(0, "Arm.L"),
            MockVertexGroup(1, "Arm.R"),
            MockVertexGroup(2, "corrective_smooth"),  # Non-deform utility group
        ]
        vertices = []
        for i in range(num_vertices):
            # Deform bone weights sum to 1.6 (> 1.0) if unconstrained
            # Also include a zero-weight entry (group 1 weight 0.0)
            if unconstrained:
                groups = [
                    MockVertexGroupElement(0, 1.2),
                    MockVertexGroupElement(1, 0.4),
                    MockVertexGroupElement(2, 0.5),  # utility group
                ]
            else:
                groups = [
                    MockVertexGroupElement(0, 0.6),
                    MockVertexGroupElement(1, 0.4),
                ]
            vertices.append(MockVertex(groups))
        self.data = MockMeshData(vertices)


def test_normalize_vertex_weights_correctness_and_performance():
    """Verify _normalize_vertex_weights clamps deform weight sum <= 1.0, preserves zero-weights, and stays < 50ms for 50,000 vertices."""
    num_verts = 50000
    mock_obj = MockObject(num_vertices=num_verts, unconstrained=True)

    start_time = time.perf_counter()
    finalize._normalize_vertex_weights(mock_obj)
    elapsed_ms = (time.perf_counter() - start_time) * 1000.0

    print(f"Normalization runtime for {num_verts} vertices: {elapsed_ms:.2f} ms")
    assert elapsed_ms < 50.0, f"Performance overhead took {elapsed_ms:.2f} ms, exceeding 50 ms limit!"

    # Verify no vertex deform weight sum > 1.0
    deform_indices = {0, 1}
    for v in mock_obj.data.vertices[:1000]:  # Check sample
        deform_sum = sum(g.weight for g in v.groups if g.group in deform_indices)
        assert deform_sum == pytest.approx(1.0)
        # Check non-deform group was untouched
        utility_w = next(g.weight for g in v.groups if g.group == 2)
        assert utility_w == 0.5


def test_normalize_vertex_weights_preserves_zero_entries():
    """Verify pre-armature weight normalization does not remove zero-weight entries."""
    v_elements = [
        MockVertexGroupElement(0, 0.8),
        MockVertexGroupElement(1, 0.0),  # Zero weight entry
        MockVertexGroupElement(2, 0.8),
    ]
    mock_obj = MockObject(num_vertices=1)
    mock_obj.vertex_groups = [
        MockVertexGroup(0, "Bone1"),
        MockVertexGroup(1, "Bone2"),
        MockVertexGroup(2, "Bone3"),
    ]
    mock_obj.data.vertices = [MockVertex(v_elements)]

    finalize._normalize_vertex_weights(mock_obj)

    v = mock_obj.data.vertices[0]
    assert len(v.groups) == 3
    # Group 1 weight was 0.0, should remain 0.0 and present
    assert v.groups[1].group == 1
    assert v.groups[1].weight == 0.0
    # Total deform sum across all 3 groups should be 1.0 (0.8 + 0.0 + 0.8 = 1.6 -> scaled by 1/1.6 = 0.5 + 0.0 + 0.5 = 1.0)
    total_sum = sum(g.weight for g in v.groups)
    assert total_sum == pytest.approx(1.0)
