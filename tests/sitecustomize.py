import sys
import types
import numpy as np

# Injected mock modules for headless testing
for mod_name in ['addon_utils', 'rna_prop_ui', 'gpu', 'gpu_extras', 'idprop']:
    if mod_name not in sys.modules:
        m = types.ModuleType(mod_name)
        if mod_name == 'rna_prop_ui':
            m.rna_idprop_ui_create = lambda *args, **kwargs: None
        sys.modules[mod_name] = m

if 'bpy' not in sys.modules or not hasattr(sys.modules['bpy'], 'app'):
    bpy_mock = types.ModuleType('bpy')
    wm_mock = types.SimpleNamespace()
    ui_mock = types.SimpleNamespace()
    ui_mock.fitting_binder = "SOFT"
    ui_mock.fitting_weights = "NONE"
    ui_mock.fitting_weights_ovr = True
    ui_mock.fitting_mask = "NONE"
    ui_mock.hair_deform = False
    wm_mock.charmorph_ui = ui_mock
    bpy_mock.context = types.SimpleNamespace(window_manager=wm_mock)

    class MockOperator:
        pass

    class MockPanel:
        pass

    class MockPropertyGroup:
        pass

    bpy_mock.types = types.SimpleNamespace(
        Object=object,
        Operator=MockOperator,
        Panel=MockPanel,
        PropertyGroup=MockPropertyGroup,
        AddonPreferences=object,
        ColorManagedInputColorspaceSettings=types.SimpleNamespace(
            bl_rna=types.SimpleNamespace(
                properties={"name": types.SimpleNamespace(enum_items=[])}
            )
        )
    )

    app_mock = types.ModuleType('bpy.app')
    handlers_mock = types.ModuleType('bpy.app.handlers')
    handlers_mock.persistent = lambda fn: fn
    handlers_mock.load_post = []
    app_mock.handlers = handlers_mock
    bpy_mock.app = app_mock

    props_mock = types.ModuleType('bpy.props')
    props_mock.StringProperty = lambda **kwargs: None
    props_mock.BoolProperty = lambda **kwargs: None
    props_mock.IntProperty = lambda **kwargs: None
    props_mock.FloatProperty = lambda **kwargs: None
    props_mock.EnumProperty = lambda **kwargs: None
    props_mock.PointerProperty = lambda **kwargs: None
    bpy_mock.props = props_mock

    utils_mock = types.ModuleType('bpy.utils')
    utils_mock.register_class = lambda cls: None
    utils_mock.unregister_class = lambda cls: None
    bpy_mock.utils = utils_mock

    ops_mock = types.ModuleType('bpy.ops')
    ops_mock.ed = types.SimpleNamespace()
    bpy_mock.ops = ops_mock

    sys.modules['bpy'] = bpy_mock
    sys.modules['bpy.app'] = app_mock
    sys.modules['bpy.app.handlers'] = handlers_mock
    sys.modules['bpy.props'] = props_mock
    sys.modules['bpy.utils'] = utils_mock
    sys.modules['bpy.ops'] = ops_mock

if 'bmesh' not in sys.modules:
    bmesh_mock = types.ModuleType('bmesh')
    sys.modules['bmesh'] = bmesh_mock

if 'mathutils' not in sys.modules:
    mathutils_mock = types.ModuleType('mathutils')

    class Vector(np.ndarray):
        def __new__(cls, input_array):
            obj = np.asarray(input_array, dtype=np.float64).view(cls)
            return obj

        @property
        def length(self):
            return float(np.linalg.norm(self))

        def tolist(self):
            return super().tolist()

    class MockQuaternion(np.ndarray):
        def __new__(cls, *args):
            return np.array([1.0, 0.0, 0.0, 0.0], dtype=np.float64).view(cls)

    class MockEuler(np.ndarray):
        def __new__(cls, *args):
            return np.array([0.0, 0.0, 0.0], dtype=np.float64).view(cls)

    class MockMatrix(np.ndarray):
        def __new__(cls, *args):
            return np.eye(4, dtype=np.float64).view(cls)
        def to_3x3(self):
            return np.eye(3, dtype=np.float64)

    class MockBVHTree:
        def __init__(self, verts=None, faces=None):
            self.verts = np.array(verts, dtype=np.float64) if verts is not None else np.array([])
            self.faces = faces or []

        @classmethod
        def FromPolygons(cls, verts, faces):
            return cls(verts, faces)

        @classmethod
        def FromBMesh(cls, bm):
            return cls()

        def find_nearest(self, co, max_dist=1e30):
            co = np.array(co, dtype=np.float64)
            if len(self.verts) == 0:
                return None, None, None, None
            dists = np.linalg.norm(self.verts - co, axis=1)
            min_idx = int(np.argmin(dists))
            min_dist = float(dists[min_idx])
            if min_dist > max_dist:
                return None, None, None, None
            face_idx = 0
            for fi, f in enumerate(self.faces):
                if min_idx in f:
                    face_idx = fi
                    break
            f = self.faces[face_idx] if self.faces else [min_idx]
            if len(f) >= 3:
                v0, v1, v2 = self.verts[f[0]], self.verts[f[1]], self.verts[f[2]]
                fn = np.cross(v1 - v0, v2 - v0)
                norm = np.linalg.norm(fn)
                if norm > 1e-12:
                    fn = fn / norm
                else:
                    fn = np.array([0.0, 0.0, 1.0])
            else:
                fn = np.array([0.0, 0.0, 1.0])
            loc = self.verts[min_idx]
            return Vector(loc), Vector(fn), face_idx, min_dist

        def find_nearest_range(self, co, max_dist):
            res = self.find_nearest(co, max_dist)
            if res[0] is None:
                return []
            return [res]

        def ray_cast(self, co, direction, max_dist=1e30):
            return None, None, None, None

    class MockInterpolate:
        @staticmethod
        def poly_3d_calc(verts, loc):
            n = len(verts)
            return [1.0 / n] * n

    mathutils_mock.Vector = Vector
    mathutils_mock.Quaternion = MockQuaternion
    mathutils_mock.Euler = MockEuler
    mathutils_mock.Matrix = MockMatrix
    mathutils_mock.bvhtree = types.SimpleNamespace(BVHTree=MockBVHTree)
    mathutils_mock.interpolate = MockInterpolate
    sys.modules['mathutils'] = mathutils_mock
    sys.modules['mathutils.bvhtree'] = mathutils_mock.bvhtree
    sys.modules['mathutils.interpolate'] = mathutils_mock.interpolate
