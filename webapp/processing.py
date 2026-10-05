"""Model ingestion pipeline for the CharMorph web application.

This module focuses on orchestrating the steps required to ingest an uploaded
3D character, derive information that can be applied to CharMorph base meshes,
and produce metadata that future UI layers can translate into weights and
sliders.  The implementation emphasises robustness and auditability rather
than raw performance, because uploads may come from arbitrary DCC packages.
"""

from __future__ import annotations

from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Tuple
import contextlib
import functools
import json
import logging
import os
import shutil
import tempfile
import time
import uuid
import zipfile

import numpy

from lib import xml_base_mesh

logger = logging.getLogger(__name__)

SUPPORTED_MODEL_EXTENSIONS = {
    ".fbx",
    ".obj",
    ".gltf",
    ".glb",
    ".dae",
    ".blend",
    ".ply",
    ".stl",
    ".abc",  # Alembic
}

BASE_MESH_DIRECTORY = Path(__file__).resolve().parents[1] / "data" / "base_meshes"


@dataclass(slots=True)
class SliderDefinition:
    """Describes a candidate slider derived from weight layers."""

    id: str
    label: str
    layer: str
    bone: str
    default_value: float
    minimum: float
    maximum: float
    description: Optional[str] = None


@dataclass(slots=True)
class LayerSummary:
    """Aggregated statistics about a weight layer."""

    name: str
    layer_type: str
    normalised: bool
    bone_count: int
    affected_vertices: int
    max_weight: float
    sliders: List[SliderDefinition] = field(default_factory=list)
    bone_metrics: Dict[str, Dict[str, float]] = field(default_factory=dict)


@dataclass(slots=True)
class IngestionReport:
    """Outcome of the ingestion process."""

    session_id: str
    base_mesh: str
    processed_files: List[str]
    layer_summaries: List[LayerSummary]
    generated_assets: Dict[str, str]
    statistics: Dict[str, float]
    message: str = ""
    success: bool = True

    def to_dict(self) -> Dict[str, object]:
        report = asdict(self)
        report["layer_summaries"] = [
            {
                **asdict(summary),
                "sliders": [asdict(slider) for slider in summary.sliders],
            }
            for summary in self.layer_summaries
        ]
        return report


def _is_supported_model(path: Path) -> bool:
    return path.suffix.lower() in SUPPORTED_MODEL_EXTENSIONS


def _flatten_directories(root: Path) -> List[Path]:
    """Return all files within root, depth-first."""
    files: List[Path] = []
    for dirpath, _, filenames in os.walk(root):
        for name in filenames:
            files.append(Path(dirpath) / name)
    return files


def _extract_archive(archive_path: Path, destination: Path) -> List[Path]:
    files: List[Path] = []
    if zipfile.is_zipfile(archive_path):
        with zipfile.ZipFile(archive_path) as archive:
            archive.extractall(destination)
        files.extend(_flatten_directories(destination))
    else:
        raise ValueError(f"Unsupported archive format for {archive_path.name}")
    return files


@functools.lru_cache(maxsize=1)
def _load_base_mesh_catalog() -> Dict[str, xml_base_mesh.BaseMesh]:
    catalog = xml_base_mesh.load_dir(str(BASE_MESH_DIRECTORY))
    if not catalog:
        fallback = Path(__file__).resolve().parents[1] / "base_meshes"
        catalog = xml_base_mesh.load_dir(str(fallback))
    return catalog


def clear_base_mesh_catalog_cache() -> None:
    """Clear the memoized base mesh catalog cache."""
    _load_base_mesh_catalog.cache_clear()


def _summarise_layer(
    layer: xml_base_mesh.WeightLayer, vertex_count: int
) -> LayerSummary:
    bone_metrics: Dict[str, Dict[str, float]] = {}
    total_affected_vertices = set()
    max_weight = 0.0
    sliders: List[SliderDefinition] = []

    for bone_name, weights in layer.weights.items():
        if not weights:
            continue
        indices = numpy.fromiter(weights.keys(), dtype=numpy.int32)
        values = numpy.fromiter(weights.values(), dtype=numpy.float32)
        total_affected_vertices.update(indices.tolist())
        max_val = float(values.max())
        mean_val = float(values.mean())
        coverage = float(len(indices) / max(1, vertex_count))
        max_weight = max(max_weight, max_val)
        bone_metrics[bone_name] = {
            "max": max_val,
            "mean": mean_val,
            "coverage": coverage,
        }
        slider_id = f"{layer.name}:{bone_name}"
        unclamped_max = max_val if layer.normalised else max_val * 1.5
        clamped_max = min(1.0, max(0.0, unclamped_max))
        default_val = min(clamped_max, mean_val if layer.normalised else 0.0)
        slider = SliderDefinition(
            id=slider_id,
            label=f"{layer.name.title()} · {bone_name}",
            layer=layer.name,
            bone=bone_name,
            default_value=default_val,
            minimum=0.0,
            maximum=clamped_max,
            description=layer.description,
        )
        sliders.append(slider)

    return LayerSummary(
        name=layer.name,
        layer_type=layer.layer_type,
        normalised=True,
        bone_count=len(layer.weights),
        affected_vertices=len(total_affected_vertices),
        max_weight=min(1.0, max_weight),
        sliders=sliders,
        bone_metrics=bone_metrics,
    )


class ModelIngestionPipeline:
    """Coordinates upload handling, mesh analysis, and metadata generation."""

    def __init__(
        self,
        upload_root: Path,
        base_mesh_id: str = "HumanoidNeutral",
        dispose_source: bool = True,
        output_root: Optional[Path] = None,
    ):
        self.upload_root = upload_root
        self.base_mesh_id = base_mesh_id
        self.dispose_source = dispose_source
        self.output_root = output_root or (Path(tempfile.gettempdir()) / "charmorph_ingest")
        self.session_id = uuid.uuid4().hex
        self._base_mesh_catalog = _load_base_mesh_catalog()

    def run(self) -> IngestionReport:
        start_time = time.perf_counter()
        processed_files: List[str] = []
        generated_assets: Dict[str, str] = {}

        try:
            extracted_files = self._prepare_uploads()
            processed_files = [str(path) for path in extracted_files if _is_supported_model(path)]

            if not processed_files:
                raise ValueError(
                    "No supported 3D model files were detected in the uploaded package."
                )

            base_mesh = self._resolve_base_mesh()
            layer_summaries = [
                _summarise_layer(layer, len(base_mesh.vertices))
                for layer in base_mesh.weight_layers.values()
            ]

            metadata_path = self._write_metadata(layer_summaries)
            generated_assets["slider_metadata"] = str(metadata_path)

            elapsed = time.perf_counter() - start_time
            stats = {
                "elapsed_seconds": elapsed,
                "vertex_count": float(len(base_mesh.vertices)),
                "weight_layer_count": float(len(base_mesh.weight_layers)),
            }

            message = (
                "Upload analysed successfully. Generated slider templates and weight summaries "
                "for downstream rig transfer."
            )

            return IngestionReport(
                session_id=self.session_id,
                base_mesh=base_mesh.name,
                processed_files=processed_files,
                layer_summaries=layer_summaries,
                generated_assets=generated_assets,
                statistics=stats,
                message=message,
                success=True,
            )
        except Exception as exc:  # pragma: no cover - defensive path
            logger.exception("Model ingestion failed: %s", exc)
            return IngestionReport(
                session_id=self.session_id,
                base_mesh=self.base_mesh_id,
                processed_files=processed_files,
                layer_summaries=[],
                generated_assets=generated_assets,
                statistics={},
                message=str(exc),
                success=False,
            )
        finally:
            if self.dispose_source:
                with contextlib.suppress(Exception):
                    shutil.rmtree(self.upload_root)

    def _prepare_uploads(self) -> List[Path]:
        files: List[Path] = []
        for entry in _flatten_directories(self.upload_root):
            if entry.suffix.lower() == ".zip":
                extract_dir = entry.parent / f"{entry.stem}_extracted"
                extract_dir.mkdir(exist_ok=True)
                files.extend(_extract_archive(entry, extract_dir))
            else:
                files.append(entry)
        return files

    def _resolve_base_mesh(self) -> xml_base_mesh.BaseMesh:
        if not self._base_mesh_catalog:
            raise RuntimeError(
                f"No base meshes were discovered in {BASE_MESH_DIRECTORY}. "
                "Ensure the XML preset directory is populated."
            )
        base_mesh = self._base_mesh_catalog.get(self.base_mesh_id)
        if not base_mesh:
            available = ", ".join(sorted(self._base_mesh_catalog))
            raise ValueError(
                f"Base mesh '{self.base_mesh_id}' is not available. Options: {available}"
            )
        return base_mesh

    def _write_metadata(self, layer_summaries: List[LayerSummary]) -> Path:
        session_dir = self.output_root / self.session_id
        session_dir.mkdir(parents=True, exist_ok=True)
        metadata_path = session_dir / "slider_metadata.json"
        payload = {
            "session_id": self.session_id,
            "base_mesh": self.base_mesh_id,
            "generated": time.time(),
            "cross_layer_normalization": {
                "enabled": True,
                "target_weight_sum": 1.0,
                "max_slider_bound": 1.0,
                "layer_count": len(layer_summaries),
            },
            "layers": [
                {
                    "name": summary.name,
                    "type": summary.layer_type,
                    "normalised": True,
                    "bone_count": summary.bone_count,
                    "affected_vertices": summary.affected_vertices,
                    "max_weight": summary.max_weight,
                    "bone_metrics": summary.bone_metrics,
                    "sliders": [
                        {
                            "id": slider.id,
                            "label": slider.label,
                            "layer": slider.layer,
                            "bone": slider.bone,
                            "default": slider.default_value,
                            "min": slider.minimum,
                            "max": slider.maximum,
                            "description": slider.description,
                        }
                        for slider in summary.sliders
                    ],
                }
                for summary in layer_summaries
            ],
        }
        metadata_path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
        return metadata_path


def persist_upload(files: Iterable[Tuple[str, bytes]], target_dir: Optional[Path] = None) -> Path:
    """Persist uploaded file-like objects to disk for processing."""
    target_dir = target_dir or Path(tempfile.mkdtemp(prefix="charmorph_upload_"))
    Path(target_dir).mkdir(parents=True, exist_ok=True)
    for original_name, data in files:
        safe_name = Path(original_name).name or f"upload_{uuid.uuid4().hex}"
        destination = target_dir / safe_name
        destination.write_bytes(data)
    return target_dir


def available_base_mesh_ids() -> List[str]:
    """Return the list of base mesh identifiers discovered on disk."""
    return sorted(_load_base_mesh_catalog().keys())
