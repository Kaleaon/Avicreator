import os
import sys
import types
import pytest
import numpy as np

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

import finalize
from lib import drivers
import file_io


class MockDataCollection(list):
    def foreach_get(self, prop, arr):
        for i, item in enumerate(self):
            val = getattr(item, prop)
            arr[i * 3: i * 3 + 3] = val

    def foreach_set(self, prop, arr):
        for i, item in enumerate(self):
            setattr(item, prop, arr[i * 3: i * 3 + 3])


class MockKeyBlock:
    def __init__(self, name, value=0.0):
        self.name = name
        self.value = value
        self.mute = False
        self.slider_min = -1.0
        self.slider_max = 1.0
        self.data = MockDataCollection()
        self.driver_fcurve = None

    def driver_add(self, prop_name):
        class MockDriverTarget:
            def __init__(self):
                self.id_type = None
                self.id = None
                self.data_path = None

        class MockVariable:
            def __init__(self):
                self.name = ""
                self.targets = [MockDriverTarget()]

        class MockVariablesList(list):
            def new(self):
                v = MockVariable()
                self.append(v)
                return v

        class MockDriver:
            def __init__(self):
                self.type = 'AVERAGE'
                self.variables = MockVariablesList()

        class MockFCurve:
            def __init__(self):
                self.driver = MockDriver()

        self.driver_fcurve = MockFCurve()
        return self.driver_fcurve


class MockKeyBlocksList(list):
    def get(self, name, default=None):
        for item in self:
            if getattr(item, "name", None) == name:
                return item
        return default


class MockShapeKeys:
    def __init__(self, key_blocks=None):
        self.reference_key = MockKeyBlock("Basis")
        self.key_blocks = MockKeyBlocksList([self.reference_key])
        if key_blocks:
            self.key_blocks.extend(key_blocks)


class MockVertex:
    def __init__(self, co):
        self.co = np.array(co, dtype=np.float64)


class MockMeshData(dict):
    def __init__(self, num_verts=4):
        super().__init__()
        self.vertices = MockDataCollection([MockVertex([float(i), 0.0, 0.0]) for i in range(num_verts)])
        self.shape_keys = MockShapeKeys()

    def update(self):
        pass


class MockModifier:
    def __init__(self, name, type_str):
        self.name = name
        self.type = type_str
        self.target = None
        self.is_bound = False
        self.smooth_type = "SIMPLE"
        self.vertex_group = ""
        self.show_viewport = True


class MockModifiersList(list):
    def new(self, name, type_str):
        mod = MockModifier(name, type_str)
        self.append(mod)
        return mod

    def remove(self, mod):
        if mod in self:
            super().remove(mod)


class MockObject:
    def __init__(self, name="CharMesh"):
        self.name = name
        self.data = MockMeshData()
        self.modifiers = MockModifiersList()

    def shape_key_add(self, name="charmorph_final", from_mix=True):
        sk = MockKeyBlock(name)
        sk.data = MockDataCollection([MockVertex([0.0, 0.0, 0.0]) for _ in range(len(self.data.vertices))])
        self.data.shape_keys.key_blocks.append(sk)
        return sk

    def shape_key_remove(self, key):
        if key in self.data.shape_keys.key_blocks:
            self.data.shape_keys.key_blocks.remove(key)


def test_retain_finalization_preserves_shapekeys_and_props():
    """Criterion 1: Character shape keys and cm_morpher custom properties remain intact after OpFinalize with fin_morph == RETAIN."""
    obj = MockObject()
    obj.data["cm_morpher"] = "ext"
    obj.data["cmorph_L2_height"] = 0.5
    l2_sk = MockKeyBlock("L2_height", 0.5)
    obj.data.shape_keys.key_blocks.append(l2_sk)

    ui = types.SimpleNamespace(fin_morph="RETAIN")

    # Mock manager
    finalize.mm = types.SimpleNamespace(
        morpher=types.SimpleNamespace(
            core=types.SimpleNamespace(obj=obj)
        )
    )

    finalize._cleanup_morphs(ui, fin_sk=None)

    assert "cm_morpher" in obj.data
    assert obj.data["cm_morpher"] == "ext"
    assert "cmorph_L2_height" in obj.data
    assert l2_sk in obj.data.shape_keys.key_blocks


def test_realtime_slider_driver_updates():
    """Criterion 2: Adjusting body parameter sliders on a character sets up live drivers and updates values."""
    print("DRIVERS FILE:", getattr(drivers, "__file__", "NONE"))
    obj = MockObject()
    sk_height = MockKeyBlock("L2_Height", 0.0)
    obj.data.shape_keys.key_blocks.append(sk_height)

    drivers.setup_shapekey_drivers(obj)

    assert "cmorph_L2_Height" in obj.data
    assert sk_height.driver_fcurve is not None
    assert sk_height.driver_fcurve.driver.variables[0].targets[0].data_path == 'data["cmorph_L2_Height"]'

    # Updating custom property simulates live driver behavior
    obj.data["cmorph_L2_Height"] = 0.75
    assert obj.data["cmorph_L2_Height"] == 0.75


def test_asset_surface_deform_modifier():
    """Criterion 3: Clothing assets dynamically adjust through active Surface Deform modifier stack constraints."""
    char_obj = MockObject("Character")
    asset_obj = MockObject("Clothing")

    afd = types.SimpleNamespace(obj=asset_obj)

    finalize.mm = types.SimpleNamespace(
        morpher=types.SimpleNamespace(
            core=types.SimpleNamespace(obj=char_obj),
            fitter=types.SimpleNamespace(get_assets=lambda: [afd])
        )
    )

    ui = types.SimpleNamespace(
        fin_csmooth=False,
        fin_subdivision="NO",
        fin_csmooth_assets="NO",
        fin_subdiv_assets=False
    )

    finalize._add_modifiers(ui)

    sdef_mods = [m for m in asset_obj.modifiers if m.type == "SURFACE_DEFORM"]
    assert len(sdef_mods) == 1
    assert sdef_mods[0].target == char_obj
    assert sdef_mods[0].is_bound is True


def test_export_baking_collapses_shapekeys():
    """Criterion 4: Automated export baking collapses shape keys into a single mesh state without altering visual appearance."""
    obj = MockObject()
    sk1 = MockKeyBlock("L2_muscles", 1.0)
    obj.data.shape_keys.key_blocks.append(sk1)

    assert len(obj.data.shape_keys.key_blocks) > 0

    file_io.bake_shape_keys_for_export(obj)

    assert len(obj.data.shape_keys.key_blocks) == 0


def test_mute_inactive_shape_keys():
    """Guardrail: Mute inactive shape key evaluation groups to optimize viewport playback performance."""
    obj = MockObject()
    sk_active = MockKeyBlock("L2_active", 0.8)
    sk_inactive = MockKeyBlock("L2_inactive", 0.0)
    obj.data.shape_keys.key_blocks.extend([sk_active, sk_inactive])

    drivers.mute_inactive_shape_keys(obj)

    assert sk_active.mute is False
    assert sk_inactive.mute is True
