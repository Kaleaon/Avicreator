import sys
from unittest.mock import MagicMock

class MockModule(MagicMock):
    @property
    def __path__(self):
        return []

class DummyOperator:
    pass

class DummyPanel:
    pass

class DummyUIList:
    pass

class DummyHeader:
    pass

class DummyMenu:
    pass

class DummyPropertyGroup:
    pass

class DummyAddonPreferences:
    pass

class DummyObject:
    pass

class DummyImportHelper:
    pass

class DummyExportHelper:
    pass

class DummyPropertyDeferred:
    pass

if "bpy" not in sys.modules:
    mock_bpy = MockModule()
    mock_bpy.app = MockModule()
    mock_bpy.app.version = (2, 90, 0)
    mock_bpy.app.handlers = MockModule()
    mock_bpy.props = MockModule()
    mock_bpy.props._PropertyDeferred = DummyPropertyDeferred

    mock_bpy.types = MockModule()
    mock_bpy.types.Operator = DummyOperator
    mock_bpy.types.Panel = DummyPanel
    mock_bpy.types.UIList = DummyUIList
    mock_bpy.types.Header = DummyHeader
    mock_bpy.types.Menu = DummyMenu
    mock_bpy.types.PropertyGroup = DummyPropertyGroup
    mock_bpy.types.AddonPreferences = DummyAddonPreferences
    mock_bpy.types.Object = DummyObject

    mock_bpy.utils = MockModule()
    mock_bpy.utils.register_classes_factory = lambda classes: (lambda: None, lambda: None)

    mock_bpy_extras = MockModule()
    mock_bpy_extras.wm_utils = MockModule()
    mock_bpy_extras.wm_utils.progress_report = MockModule()
    mock_bpy_extras.io_utils = MockModule()
    mock_bpy_extras.io_utils.ImportHelper = DummyImportHelper
    mock_bpy_extras.io_utils.ExportHelper = DummyExportHelper

    sys.modules["bpy"] = mock_bpy
    sys.modules["bpy.app"] = mock_bpy.app
    sys.modules["bpy.app.handlers"] = mock_bpy.app.handlers
    sys.modules["bpy.props"] = mock_bpy.props
    sys.modules["bpy.types"] = mock_bpy.types
    sys.modules["bpy.utils"] = mock_bpy.utils
    sys.modules["bpy_extras"] = mock_bpy_extras
    sys.modules["bpy_extras.wm_utils"] = mock_bpy_extras.wm_utils
    sys.modules["bpy_extras.wm_utils.progress_report"] = mock_bpy_extras.wm_utils.progress_report
    sys.modules["bpy_extras.io_utils"] = mock_bpy_extras.io_utils
    sys.modules["bmesh"] = MockModule()
    sys.modules["mathutils"] = MockModule()
    sys.modules["rna_prop_ui"] = MockModule()
    sys.modules["addon_utils"] = MockModule()
    sys.modules["idprop"] = MockModule()
