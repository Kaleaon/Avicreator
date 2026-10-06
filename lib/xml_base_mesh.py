"""Utilities for loading rigged humanoid base meshes from XML files.

The XML schema is intentionally human-readable so that technical artists can
author and version-control geometry, rig metadata, multi-layer weight maps
for skin, muscles, and fat, as well as sizing presets.  The parser converts
the XML representation into simple Python data classes that can be consumed
by Blender operators or exported to other DCC tools.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, Iterable, List, Optional, Tuple
import os
import xml.etree.ElementTree as ET

import numpy


Vector3 = Tuple[float, float, float]


def _coerce_float(value: Optional[str], default: float = 0.0) -> float:
    if value is None or value == "":
        return default
    try:
        return float(value)
    except ValueError as exc:  # pragma: no cover - defensive logging hook
        raise ValueError(f"Expected float value, got {value!r}") from exc


def _coerce_int(value: Optional[str]) -> int:
    if value is None or value == "":
        raise ValueError("Missing required integer attribute")
    try:
        return int(value)
    except ValueError as exc:  # pragma: no cover - defensive logging hook
        raise ValueError(f"Expected int value, got {value!r}") from exc


def _vector_from_attributes(node: ET.Element, keys: Iterable[str] = ("x", "y", "z")) -> Vector3:
    result = tuple(_coerce_float(node.get(key)) for key in keys)
    if len(result) != 3:
        raise ValueError(f"Vector for {node.tag} must have exactly three components")
    return result  # type: ignore[return-value]


def _split_indices(value: str) -> Tuple[int, ...]:
    if not value:
        raise ValueError("Face element requires verts attribute with at least one index")
    try:
        return tuple(int(idx) for idx in value.replace(",", " ").split())
    except ValueError as exc:
        raise ValueError(f"Invalid vertex index list: {value!r}") from exc


@dataclass(slots=True)
class Bone:
    """Simple bone definition used for rig generation."""

    name: str
    parent: Optional[str]
    head: Vector3
    tail: Vector3
    roll: float = 0.0
    inherit_scale: str = "FULL"


@dataclass(slots=True)
class WeightLayer:
    """A named collection of bone weight envelopes."""

    name: str
    layer_type: str
    normalised: bool
    description: Optional[str] = None
    weights: Dict[str, Dict[int, float]] = field(default_factory=dict)

    def as_numpy(self, vertex_count: int, normalize: bool = False) -> Dict[str, numpy.ndarray]:
        """Return weight maps as dense numpy arrays, optionally normalized."""
        arrays: Dict[str, numpy.ndarray] = {}
        for bone_name, weight_map in self.weights.items():
            array = numpy.zeros(vertex_count, dtype=numpy.float32)
            for vidx, value in weight_map.items():
                if vidx < 0 or vidx >= vertex_count:
                    raise IndexError(
                        f"Weight index {vidx} for bone '{bone_name}' outside vertex range 0..{vertex_count - 1}"
                    )
                array[vidx] = value
            arrays[bone_name] = array

        if normalize and arrays:
            stacked = numpy.stack(list(arrays.values()), axis=0)
            sums = stacked.sum(axis=0)
            mask = sums > 1.0
            if numpy.any(mask):
                stacked[:, mask] /= sums[mask]
                for i, bone_name in enumerate(arrays.keys()):
                    arrays[bone_name] = stacked[i]

        return arrays

    def as_normalized_numpy(self, vertex_count: int) -> Dict[str, numpy.ndarray]:
        """Return weight maps as dense numpy arrays with weight sums capped at 1.0."""
        return self.as_numpy(vertex_count, normalize=True)


@dataclass(slots=True)
class SizingParameter:
    """Defines a parametrised sizing control (e.g. height, weight, chest)."""

    name: str
    value: float
    unit: str = ""
    minimum: Optional[float] = None
    maximum: Optional[float] = None
    description: Optional[str] = None


@dataclass(slots=True)
class BaseMesh:
    """High-level representation of a rigged base mesh."""

    name: str
    version: str
    metadata: Dict[str, str]
    vertices: List[Vector3]
    faces: List[Tuple[int, ...]]
    bones: Dict[str, Bone]
    weight_layers: Dict[str, WeightLayer]
    sizing: Dict[str, SizingParameter]
    unit: str = "meters"
    is_super_mesh: bool = False
    preallocated_geometry: Dict[str, List[int]] = field(default_factory=dict)
    limb_chains: Dict[str, List[str]] = field(default_factory=dict)

    @property
    def vertex_array(self) -> numpy.ndarray:
        return numpy.asarray(self.vertices, dtype=numpy.float32)

    @property
    def triangle_indices(self) -> numpy.ndarray:
        """Return triangulated faces (fans for ngons) as numpy array."""
        tris: List[Tuple[int, int, int]] = []
        for face in self.faces:
            if len(face) < 3:
                continue
            v0 = face[0]
            for i in range(1, len(face) - 1):
                tris.append((v0, face[i], face[i + 1]))
        return numpy.asarray(tris, dtype=numpy.int32)

    def layer(self, name: str) -> Optional[WeightLayer]:
        return self.weight_layers.get(name)

    def get_normalized_layer_weights(self, layer_name: str) -> Optional[Dict[str, numpy.ndarray]]:
        """Return normalized evaluation arrays for a named weight layer without modifying raw weights."""
        weight_layer = self.layer(layer_name)
        if weight_layer is None:
            return None
        return weight_layer.as_normalized_numpy(len(self.vertices))


def _parse_metadata(node: Optional[ET.Element]) -> Dict[str, str]:
    data: Dict[str, str] = {}
    if node is None:
        return data
    for child in node:
        key = child.tag.strip()
        if not key:
            continue
        data[key] = (child.text or "").strip()
    return data


def _parse_vertices(node: ET.Element) -> List[Vector3]:
    vertices: List[Vector3] = []
    for vertex_node in node.findall("Vertex"):
        vidx = _coerce_int(vertex_node.get("id"))
        co = _vector_from_attributes(vertex_node)
        if vidx != len(vertices):
            if vidx < len(vertices):
                raise ValueError(f"Duplicate vertex id {vidx}")
            # pad missing indices with zeros to keep array positions stable
            while len(vertices) < vidx:
                vertices.append((0.0, 0.0, 0.0))
        vertices.append(co)
    return vertices


def _parse_faces(node: Optional[ET.Element]) -> List[Tuple[int, ...]]:
    faces: List[Tuple[int, ...]] = []
    if node is None:
        return faces
    for face_node in node.findall("Face"):
        indices = _split_indices(face_node.get("verts", ""))
        if len(indices) < 3:
            raise ValueError("Face must contain at least three indices")
        faces.append(indices)
    return faces


def _parse_rig(node: Optional[ET.Element]) -> Dict[str, Bone]:
    bones: Dict[str, Bone] = {}
    if node is None:
        return bones
    for bone_node in node.findall("Bone"):
        name = bone_node.get("name")
        if not name:
            raise ValueError("Bone missing required 'name' attribute")
        parent = bone_node.get("parent") or None
        head = _vector_from_attributes(bone_node, ("head_x", "head_y", "head_z")) if "head_x" in bone_node.attrib else None
        tail = _vector_from_attributes(bone_node, ("tail_x", "tail_y", "tail_z")) if "tail_x" in bone_node.attrib else None
        if head is None:
            head = _vector_from_attributes(bone_node, ("headX", "headY", "headZ")) if "headX" in bone_node.attrib else None
        if tail is None:
            tail = _vector_from_attributes(bone_node, ("tailX", "tailY", "tailZ")) if "tailX" in bone_node.attrib else None
        if head is None:
            head = _vector_from_attributes(bone_node)
        if tail is None:
            tail = _vector_from_attributes(bone_node, ("tail_x", "tail_y", "tail_z"))
        roll = _coerce_float(bone_node.get("roll"), 0.0)
        inherit_scale = bone_node.get("inheritScale", "FULL")
        bones[name] = Bone(name=name, parent=parent, head=head, tail=tail, roll=roll, inherit_scale=inherit_scale)
    return bones


def _parse_weight_layers(node: Optional[ET.Element], vertex_count: int) -> Dict[str, WeightLayer]:
    layers: Dict[str, WeightLayer] = {}
    if node is None:
        return layers
    for layer_node in node.findall("Layer"):
        name = layer_node.get("name")
        if not name:
            raise ValueError("Weight Layer missing required 'name' attribute")
        layer_type = layer_node.get("type", "generic")
        normalised = (layer_node.get("normalised", "true").lower() != "false")
        description = (layer_node.findtext("Description") or "").strip() or None
        weight_layer = WeightLayer(name=name, layer_type=layer_type, normalised=normalised, description=description)
        for bone_node in layer_node.findall("Bone"):
            bone_name = bone_node.get("name")
            if not bone_name:
                raise ValueError(f"Weight layer '{name}' contains bone without name")
            bone_weights: Dict[int, float] = {}
            for weight_node in bone_node.findall("Weight"):
                vidx = _coerce_int(weight_node.get("vertex"))
                value = _coerce_float(weight_node.get("value"))
                if vidx < 0 or vidx >= vertex_count:
                    raise IndexError(
                        f"Weight index {vidx} for bone '{bone_name}' outside vertex range 0..{vertex_count - 1}"
                    )
                bone_weights[vidx] = value
            if bone_weights:
                weight_layer.weights[bone_name] = bone_weights
        layers[name] = weight_layer
    return layers


def _parse_sizing(node: Optional[ET.Element]) -> Dict[str, SizingParameter]:
    sizing: Dict[str, SizingParameter] = {}
    if node is None:
        return sizing
    for param_node in node.findall("Parameter"):
        name = param_node.get("name")
        if not name:
            raise ValueError("Sizing Parameter missing required 'name' attribute")
        value = _coerce_float(param_node.get("value"))
        unit = param_node.get("unit", "")
        minimum_attr = param_node.get("min")
        maximum_attr = param_node.get("max")
        sizing[name] = SizingParameter(
            name=name,
            value=value,
            unit=unit,
            minimum=_coerce_float(minimum_attr) if minimum_attr else None,
            maximum=_coerce_float(maximum_attr) if maximum_attr else None,
            description=(param_node.text or "").strip() or None,
        )
    return sizing


def _parse_preallocated_geometry(node: Optional[ET.Element]) -> Dict[str, List[int]]:
    result: Dict[str, List[int]] = {}
    if node is None:
        return result
    for region_node in node.findall("Region") + node.findall("Group"):
        name = region_node.get("name")
        if not name:
            continue
        indices_str = region_node.get("indices") or region_node.get("verts") or region_node.text or ""
        if indices_str:
            result[name] = list(_split_indices(indices_str.strip()))
    return result


def _parse_limb_chains(node: Optional[ET.Element]) -> Dict[str, List[str]]:
    result: Dict[str, List[str]] = {}
    if node is None:
        return result
    for chain_node in node.findall("Chain") + node.findall("Limb"):
        name = chain_node.get("name")
        if not name:
            continue
        bones_str = chain_node.get("bones") or chain_node.text or ""
        if bones_str:
            result[name] = [b.strip() for b in bones_str.replace(",", " ").split() if b.strip()]
    return result


def load_base_mesh(path: str) -> BaseMesh:
    """Load a single XML base mesh definition."""
    tree = ET.parse(path)
    root = tree.getroot()
    if root.tag not in ("BaseMesh", "SuperMesh"):
        raise ValueError(f"Root element must be <BaseMesh> or <SuperMesh>, got <{root.tag}> in {path}")

    is_super_mesh = (
        root.tag == "SuperMesh"
        or root.get("type") == "super_mesh"
        or root.get("is_super_mesh", "").lower() == "true"
    )

    name = root.get("name") or os.path.splitext(os.path.basename(path))[0]
    version = root.get("version", "1.0")
    metadata = _parse_metadata(root.find("Metadata"))

    topology_node = root.find("Topology")
    if topology_node is None:
        raise ValueError(f"<Topology> element missing in {path}")
    unit = topology_node.get("unit", "meters")

    vertices_node = topology_node.find("Vertices")
    if vertices_node is None:
        raise ValueError(f"<Vertices> element missing in {path}")
    vertices = _parse_vertices(vertices_node)
    faces = _parse_faces(topology_node.find("Faces"))

    bones = _parse_rig(root.find("Rig"))
    weight_layers = _parse_weight_layers(root.find("WeightLayers"), len(vertices))
    sizing = _parse_sizing(root.find("Sizing"))

    preallocated_geom_node = root.find("PreallocatedGeometry")
    if preallocated_geom_node is None:
        preallocated_geom_node = root.find("NonHumanoidGeometry")
    if preallocated_geom_node is None and topology_node is not None:
        preallocated_geom_node = topology_node.find("PreallocatedGeometry")
        if preallocated_geom_node is None:
            preallocated_geom_node = topology_node.find("NonHumanoidGeometry")
    preallocated_geometry = _parse_preallocated_geometry(preallocated_geom_node)

    limb_chains_node = root.find("LimbChains")
    if limb_chains_node is None:
        limb_chains_node = root.find("BoneChains")
    if limb_chains_node is None:
        rig_node = root.find("Rig")
        if rig_node is not None:
            limb_chains_node = rig_node.find("LimbChains")
            if limb_chains_node is None:
                limb_chains_node = rig_node.find("BoneChains")
    limb_chains = _parse_limb_chains(limb_chains_node)

    return BaseMesh(
        name=name,
        version=version,
        metadata=metadata,
        vertices=vertices,
        faces=faces,
        bones=bones,
        weight_layers=weight_layers,
        sizing=sizing,
        unit=unit,
        is_super_mesh=is_super_mesh,
        preallocated_geometry=preallocated_geometry,
        limb_chains=limb_chains,
    )


def load_dir(path: str) -> Dict[str, BaseMesh]:
    """Load all XML base meshes from the given directory."""
    result: Dict[str, BaseMesh] = {}
    if not os.path.isdir(path):
        return result
    for entry in sorted(os.listdir(path)):
        if not entry.lower().endswith(".xml"):
            continue
        full_path = os.path.join(path, entry)
        if not os.path.isfile(full_path):
            continue
        mesh = load_base_mesh(full_path)
        result[mesh.name] = mesh
    return result
