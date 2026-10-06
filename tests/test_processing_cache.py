"""Unit tests for base mesh catalog memoization and caching in webapp.processing."""

from pathlib import Path
from unittest.mock import patch
import pytest
from fastapi.testclient import TestClient

from lib import xml_base_mesh
from webapp import processing
from webapp.server import app


@pytest.fixture(autouse=True)
def reset_catalog_cache():
    """Ensure catalog cache is cleared before and after each test."""
    processing.clear_base_mesh_catalog_cache()
    yield
    processing.clear_base_mesh_catalog_cache()


def test_load_base_mesh_catalog_memoization():
    """Verify consecutive calls return the exact same cached catalog instance."""
    catalog1 = processing._load_base_mesh_catalog()
    catalog2 = processing._load_base_mesh_catalog()

    assert catalog1 is catalog2
    assert "HumanoidNeutral" in catalog1
    assert "HumanoidAthletic" in catalog1


def test_load_base_mesh_catalog_single_disk_load():
    """Verify xml_base_mesh.load_dir is called only once across multiple calls."""
    with patch.object(xml_base_mesh, "load_dir", wraps=xml_base_mesh.load_dir) as mock_load:
        processing._load_base_mesh_catalog()
        processing._load_base_mesh_catalog()
        processing._load_base_mesh_catalog()

        assert mock_load.call_count == 1


def test_clear_base_mesh_catalog_cache():
    """Verify clear_base_mesh_catalog_cache clears the cache and re-loads from disk."""
    with patch.object(xml_base_mesh, "load_dir", wraps=xml_base_mesh.load_dir) as mock_load:
        cat1 = processing._load_base_mesh_catalog()
        assert mock_load.call_count == 1

        processing.clear_base_mesh_catalog_cache()

        cat2 = processing._load_base_mesh_catalog()
        assert mock_load.call_count == 2
        assert cat1 is not cat2
        assert cat1.keys() == cat2.keys()


def test_available_base_mesh_ids():
    """Verify available_base_mesh_ids uses the cached catalog."""
    mesh_ids = processing.available_base_mesh_ids()
    assert "HumanoidNeutral" in mesh_ids
    assert "HumanoidAthletic" in mesh_ids

    with patch.object(xml_base_mesh, "load_dir", wraps=xml_base_mesh.load_dir) as mock_load:
        ids_cached = processing.available_base_mesh_ids()
        assert ids_cached == mesh_ids
        assert mock_load.call_count == 0


def test_fastapi_server_endpoints(tmp_path: Path):
    """Verify FastAPI GET /base-meshes and POST /ingest-model work with cached catalog."""
    client = TestClient(app)

    # Test GET /base-meshes
    response = client.get("/base-meshes")
    assert response.status_code == 200
    data = response.json()
    assert "items" in data
    assert "HumanoidNeutral" in data["items"]

    # Test POST /ingest-model with dummy model file
    dummy_file = tmp_path / "test_model.obj"
    dummy_file.write_text("# OBJ test file\nv 0.0 0.0 0.0\nv 1.0 1.0 1.0\nf 1 2\n")

    with open(dummy_file, "rb") as f:
        ingest_response = client.post(
            "/ingest-model",
            files={"files": ("test_model.obj", f, "text/plain")},
            data={"base_mesh_id": "HumanoidNeutral"},
        )

    assert ingest_response.status_code == 200
    report = ingest_response.json()
    assert report["success"] is True
    assert report["base_mesh"] == "HumanoidNeutral"
    assert "slider_metadata" in report["generated_assets"]
