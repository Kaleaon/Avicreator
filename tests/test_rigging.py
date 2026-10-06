from unittest.mock import MagicMock
import sys

# Mock bpy and mathutils if they are not installed
if "bpy" not in sys.modules:
    mock_bpy = MagicMock()
    mock_app = MagicMock()
    mock_app.version = (4, 0, 0)
    mock_handlers = MagicMock()
    mock_handlers.persistent = lambda f: f
    mock_app.handlers = mock_handlers
    mock_bpy.app = mock_app
    mock_bpy.utils.register_classes_factory.return_value = (lambda: None, lambda: None)

    class PropertyDeferred:
        pass

    mock_props = MagicMock()
    mock_props._PropertyDeferred = PropertyDeferred
    mock_bpy.props = mock_props

    sys.modules["bpy"] = mock_bpy
    sys.modules["bpy.app"] = mock_app
    sys.modules["bpy.app.handlers"] = mock_handlers

if "mathutils" not in sys.modules:
    mock_mathutils = MagicMock()
    sys.modules["mathutils"] = mock_mathutils

if "rna_prop_ui" not in sys.modules:
    sys.modules["rna_prop_ui"] = MagicMock()

if "addon_utils" not in sys.modules:
    sys.modules["addon_utils"] = MagicMock()

if "bmesh" not in sys.modules:
    sys.modules["bmesh"] = MagicMock()

if "idprop" not in sys.modules:
    sys.modules["idprop"] = MagicMock()

if "gpu" not in sys.modules:
    sys.modules["gpu"] = MagicMock()

if "bpy_extras" not in sys.modules:
    class DummyOperator: pass
    class DummyPanel: pass
    class DummyPropertyGroup: pass
    class DummyUIList: pass
    class DummyHeader: pass
    class DummyMenu: pass
    class DummyGizmoGroup: pass
    class DummyImportHelper: pass
    class DummyExportHelper: pass

    mock_bpy.types.Operator = DummyOperator
    mock_bpy.types.Panel = DummyPanel
    mock_bpy.types.PropertyGroup = DummyPropertyGroup
    mock_bpy.types.UIList = DummyUIList
    mock_bpy.types.Header = DummyHeader
    mock_bpy.types.Menu = DummyMenu
    mock_bpy.types.GizmoGroup = DummyGizmoGroup

    mock_bpy_extras = MagicMock()
    mock_bpy_extras.io_utils.ImportHelper = DummyImportHelper
    mock_bpy_extras.io_utils.ExportHelper = DummyExportHelper

    sys.modules["bpy_extras"] = mock_bpy_extras
    sys.modules["bpy_extras.io_utils"] = mock_bpy_extras.io_utils
    sys.modules["bpy_extras.wm_utils"] = MagicMock()
    sys.modules["bpy_extras.wm_utils.progress_report"] = MagicMock()

from lib.rigging import attach_rig, set_bone_layers


def test_attach_rig_preserve_volume_inv():
    morpher = MagicMock()
    obj = MagicMock()
    obj.vertex_groups = ["preserve_volume_inv"]
    morpher.core.obj = obj
    rig = MagicMock()

    created_modifiers = {}

    def mock_new_modifier(name, mod_type):
        mod = MagicMock()
        mod.name = name
        mod.type = mod_type
        mod.invert_vertex_group = False
        created_modifiers[name] = mod
        return mod

    obj.modifiers.new.side_effect = mock_new_modifier

    attach_rig(morpher, rig)

    assert "charmorph_rig" in created_modifiers
    assert "charmorph_rig_pv" in created_modifiers
    mod2 = created_modifiers["charmorph_rig_pv"]
    assert mod2.vertex_group == "preserve_volume_inv"
    assert mod2.invert_vertex_group is True


def test_attach_rig_preserve_volume():
    morpher = MagicMock()
    obj = MagicMock()
    obj.vertex_groups = ["preserve_volume"]
    morpher.core.obj = obj
    rig = MagicMock()

    created_modifiers = {}

    def mock_new_modifier(name, mod_type):
        mod = MagicMock()
        mod.name = name
        mod.type = mod_type
        mod.invert_vertex_group = True
        created_modifiers[name] = mod
        return mod

    obj.modifiers.new.side_effect = mock_new_modifier

    attach_rig(morpher, rig)

    assert "charmorph_rig" in created_modifiers
    assert "charmorph_rig_pv" in created_modifiers
    mod2 = created_modifiers["charmorph_rig_pv"]
    assert mod2.vertex_group == "preserve_volume"
    assert mod2.invert_vertex_group is False


def test_attach_rig_no_preserve_volume():
    morpher = MagicMock()
    obj = MagicMock()
    obj.vertex_groups = []
    morpher.core.obj = obj
    rig = MagicMock()

    created_modifiers = {}

    def mock_new_modifier(name, mod_type):
        mod = MagicMock()
        mod.name = name
        mod.type = mod_type
        created_modifiers[name] = mod
        return mod

    obj.modifiers.new.side_effect = mock_new_modifier

    attach_rig(morpher, rig)

    assert "charmorph_rig" in created_modifiers
    assert "charmorph_rig_pv" not in created_modifiers
    mod = created_modifiers["charmorph_rig"]
    assert mod.use_deform_preserve_volume is True


def test_set_bone_layers():
    rig = MagicMock()
    collection_1 = MagicMock()
    collection_2 = MagicMock()
    rig.collections = {
        "Layer1": collection_1,
        "Layer2": collection_2,
    }

    bone = MagicMock()
    val = "0:Layer1,1:Layer2"

    set_bone_layers(rig, bone, val)

    collection_1.assign.assert_called_once_with(bone)
    collection_2.assign.assert_called_once_with(bone)
