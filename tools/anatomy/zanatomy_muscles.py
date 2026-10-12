"""Samples the interior of Z-Anatomy muscle meshes for fit_muscles.py.

Run with Blender's Python module (bpy), on the Z-Anatomy Startup.blend:

    python zanatomy_muscles.py Startup.blend muscles.npz "Biceps..." ...

With no names, every mesh used by fit_muscles.py's table is sampled. Each
mesh is filled by ray parity on a regular grid; the output holds one array
of interior points per mesh, in metres with Y up, +Z forward and +X on the
character's left (the shape-program frame).

Z-Anatomy ("The libre 3D atlas of anatomy", CC-BY-SA 4.0) derives from
BodyParts3D (The Database Center for Life Science, CC-BY-SA 2.1 Japan).
The samples are a derived work: keep them out of the repository.
"""
import sys

import bpy
import numpy as np
from mathutils import Vector
from mathutils.bvhtree import BVHTree


def interior_samples(obj, depsgraph):
    """Grid points inside a closed mesh (rays along X, parity per column)."""
    evaluated = obj.evaluated_get(depsgraph)
    mesh = evaluated.to_mesh()
    verts = [obj.matrix_world @ v.co for v in mesh.vertices]
    polys = [tuple(p.vertices) for p in mesh.polygons]
    evaluated.to_mesh_clear()
    if not polys:
        return np.zeros((0, 3)), 0.0
    bvh = BVHTree.FromPolygons(verts, polys)
    lo = np.min(np.array(verts), axis=0)
    hi = np.max(np.array(verts), axis=0)
    # About 12 samples across the thinnest extent, between 1.5 and 4 mm.
    h = float(np.clip(np.min(hi - lo) / 12.0, 0.0015, 0.004))
    xs = np.arange(lo[0] + h / 2, hi[0], h)
    points = []
    for y in np.arange(lo[1] + h / 2, hi[1], h):
        for z in np.arange(lo[2] + h / 2, hi[2], h):
            origin = Vector((lo[0] - 0.01, y, z))
            hits = []
            for _ in range(64):
                loc, _normal, _index, _dist = bvh.ray_cast(origin, Vector((1, 0, 0)))
                if loc is None:
                    break
                hits.append(loc.x)
                origin = Vector((loc.x + 1e-5, y, z))
            if len(hits) < 2 or len(hits) % 2:
                continue  # misses, grazing hits or an open seam
            for a, b in zip(hits[0::2], hits[1::2]):
                for x in xs[(xs > a) & (xs < b)]:
                    points.append((x, z, -y))  # Blender Z-up -> Y-up, +Z forward
    return np.array(points, dtype=np.float32).reshape(-1, 3), h


def main():
    blend, out, *names = sys.argv[1:]
    if not names:
        sys.path.insert(0, __file__.rsplit("/", 1)[0])
        from fit_muscles import MUSCLES
        names = sorted({mesh for entry in MUSCLES for mesh in entry["meshes"]})
    bpy.context.preferences.filepaths.use_scripts_auto_execute = False
    bpy.ops.wm.open_mainfile(filepath=blend, load_ui=False, use_scripts=False)
    depsgraph = bpy.context.evaluated_depsgraph_get()
    arrays = {}
    for name in names:
        obj = bpy.data.objects.get(name)
        if obj is None or obj.type != "MESH":
            print("missing mesh:", name)
            continue
        pts, h = interior_samples(obj, depsgraph)
        print(f"{name}: {len(pts)} samples at {h * 1000:.1f} mm")
        arrays[name] = pts
        arrays[name + "#spacing"] = np.array([h], dtype=np.float32)
    np.savez_compressed(out, **arrays)


if __name__ == "__main__":
    main()
