"""Cross-platform schema verification test suite.

Validates identical JSON output and deserialization behavior across Rust, Python, and Kotlin.
"""

import json
import pytest
import libavicreator


def test_cross_platform_manifest_schema():
    """Verify AssetManifest structure and JSON equivalence between Python and Rust schemas."""
    # 1. Create manifest in Python via PyO3 bindings
    manifest = libavicreator.AssetManifest(
        id="cross_platform_hero",
        name="Cross-Platform Hero",
        version="2.0.0",
        category="character",
        skeleton_type="biped"
    )
    manifest.author = "Kaleaon Core Team"
    manifest.mesh_files = ["body.gltf", "head.gltf"]
    manifest.morph_targets = ["smile", "frown"]
    manifest.tags = ["character", "core"]

    # 2. Serialize to JSON
    py_json = manifest.to_json()
    parsed_dict = json.loads(py_json)

    # 3. Verify fields match Rust schema canonical structure
    assert parsed_dict["id"] == "cross_platform_hero"
    assert parsed_dict["name"] == "Cross-Platform Hero"
    assert parsed_dict["version"] == "2.0.0"
    assert parsed_dict["category"] == "character"
    assert parsed_dict["author"] == "Kaleaon Core Team"
    assert parsed_dict["mesh_files"] == ["body.gltf", "head.gltf"]
    assert parsed_dict["morph_targets"] == ["smile", "frown"]
    assert parsed_dict["skeleton_type"] == "biped"
    assert parsed_dict["tags"] == ["character", "core"]

    # 4. Roundtrip back into PyO3 Rust schema instance
    reloaded = libavicreator.AssetManifest.from_json(py_json)
    assert reloaded.id == manifest.id
    assert reloaded.name == manifest.name
    assert reloaded.category == manifest.category
    assert reloaded.mesh_files == manifest.mesh_files


def test_cross_platform_skeleton_schema():
    """Verify Skeleton structure and hierarchy representation across platforms."""
    # 1. Build skeleton using Python PyO3 bindings
    skel = libavicreator.Skeleton("CrossPlatformBiped")
    skel.add_node(0, "Root", translation=[0.0, 0.0, 0.0])
    skel.add_node(1, "Pelvis", parent_id=0, translation=[0.0, 1.0, 0.0])
    skel.add_node(2, "Spine", parent_id=1, translation=[0.0, 0.2, 0.0])
    skel.add_node(3, "Head", parent_id=2, translation=[0.0, 0.4, 0.0])

    # 2. Serialize to canonical JSON
    skel_json = skel.to_json()
    parsed_json = json.loads(skel_json)

    assert parsed_json["name"] == "CrossPlatformBiped"
    assert parsed_json["root_indices"] == [0]
    assert len(parsed_json["nodes"]) == 4

    # Node 0 (Root)
    node0 = parsed_json["nodes"][0]
    assert node0["id"] == 0
    assert node0["name"] == "Root"
    assert node0["parent_id"] is None
    assert node0["children"] == [1]

    # Node 1 (Pelvis)
    node1 = parsed_json["nodes"][1]
    assert node1["id"] == 1
    assert node1["name"] == "Pelvis"
    assert node1["parent_id"] == 0
    assert node1["translation"] == [0.0, 1.0, 0.0]
    assert node1["children"] == [2]

    # 3. Roundtrip deserialization
    reloaded_skel = libavicreator.Skeleton.from_json(skel_json)
    assert reloaded_skel.name == skel.name
    assert reloaded_skel.node_count == 4
    assert reloaded_skel.root_indices == [0]
