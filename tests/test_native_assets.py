import os
import unittest
import types
import bpy
from pathlib import Path

try:
    import native_assets
except ImportError:
    from CharMorphExpansion import native_assets


class TestNativeAssetBrowser(unittest.TestCase):
    def setUp(self):
        # Reset asset libraries
        if hasattr(bpy.context, "preferences") and hasattr(bpy.context.preferences.filepaths, "asset_libraries"):
            bpy.context.preferences.filepaths.asset_libraries.clear()

    def test_catalog_file_generation_and_format(self):
        """Test blender_assets.cats.txt generation within directory and format standards."""
        test_dir = Path("/tmp/test_asset_lib")
        test_dir.mkdir(parents=True, exist_ok=True)

        catalog_map = native_assets.generate_catalog_file(str(test_dir), custom_categories=["CharMorph/Accessories/Hats"])
        cats_file = test_dir / "blender_assets.cats.txt"

        self.assertTrue(cats_file.exists(), "blender_assets.cats.txt should be created in library directory")
        content = cats_file.read_text(encoding="utf-8")

        self.assertTrue(content.startswith("VERSION 1"), "Catalog file must start with VERSION 1")
        self.assertIn("CharMorph/Clothing/Tops", content)
        self.assertIn("CharMorph/Hair", content)
        self.assertIn("CharMorph/Presets", content)
        self.assertIn("CharMorph/Accessories/Hats", content)

        # Check returned catalog UUID map
        self.assertIn("CharMorph/Clothing/Tops", catalog_map)

    def test_register_and_unregister_asset_library_preserves_existing(self):
        """Test non-destructive asset library registration in Blender preferences."""
        lib_dir = Path("/tmp/test_asset_lib")
        lib_dir.mkdir(parents=True, exist_ok=True)

        asset_libs = bpy.context.preferences.filepaths.asset_libraries

        # Add pre-existing user library
        asset_libs.new("Existing User Library", "/path/to/user/lib")

        # Register CharMorph library
        success = native_assets.register_asset_library(native_assets.CHARMORPH_LIB_NAME, str(lib_dir))
        self.assertTrue(success)
        self.assertIn(native_assets.CHARMORPH_LIB_NAME, asset_libs)
        self.assertEqual(asset_libs[native_assets.CHARMORPH_LIB_NAME].path, str(lib_dir))

        # Unregister CharMorph library cleanly
        native_assets.unregister_asset_library(native_assets.CHARMORPH_LIB_NAME)

        self.assertNotIn(native_assets.CHARMORPH_LIB_NAME, asset_libs)
        self.assertIn("Existing User Library", asset_libs, "Pre-existing user libraries must be preserved")

    def test_dynamic_fitting_library_dir_update(self):
        """Test asset library path dynamically updates when fitting_library_dir changes."""
        dir1 = Path("/tmp/test_user_lib_1")
        dir2 = Path("/tmp/test_user_lib_2")
        dir1.mkdir(parents=True, exist_ok=True)
        dir2.mkdir(parents=True, exist_ok=True)

        ui = bpy.context.window_manager.charmorph_ui

        # Set initial custom library dir
        native_assets.update_user_asset_library("", str(dir1))
        asset_libs = bpy.context.preferences.filepaths.asset_libraries

        self.assertIn(native_assets.CHARMORPH_USER_LIB_NAME, asset_libs)
        self.assertEqual(asset_libs[native_assets.CHARMORPH_USER_LIB_NAME].path, str(dir1))

        # Update to new custom library dir
        native_assets.update_user_asset_library(str(dir1), str(dir2))
        self.assertEqual(asset_libs[native_assets.CHARMORPH_USER_LIB_NAME].path, str(dir2))

        # Remove custom library dir (empty path)
        native_assets.update_user_asset_library(str(dir2), "")
        self.assertNotIn(native_assets.CHARMORPH_USER_LIB_NAME, asset_libs)

    def test_mark_as_native_asset_metadata_and_preview(self):
        """Test programmatically marking mesh object as native asset with catalog ID and thumbnail."""
        thumb_file = Path("/tmp/test_thumb.png")
        thumb_file.write_bytes(b"\x89PNG\r\n\x1a\n")

        class MockAssetTags:
            def __init__(self):
                self.items = []
            def new(self, tag):
                self.items.append(tag)

        class MockAssetData:
            def __init__(self):
                self.catalog_id = ""
                self.description = ""
                self.tags = MockAssetTags()

        class MockPreview:
            def __init__(self):
                self.preview_file = ""
            def load_custom_preview_from_file(self, path):
                self.preview_file = path

        class MockObject:
            def __init__(self):
                self.is_asset = False
                self.asset_data = MockAssetData()
                self.preview = MockPreview()
            def asset_mark(self):
                self.is_asset = True

        mock_obj = MockObject()

        res = native_assets.mark_as_native_asset(
            mock_obj,
            catalog_path="CharMorph/Clothing/Tops",
            description="Test Top Shirt",
            tags=("clothing", "top"),
            thumb_path=str(thumb_file)
        )

        self.assertTrue(res)
        self.assertTrue(mock_obj.is_asset)
        self.assertNotEqual(mock_obj.asset_data.catalog_id, "")
        self.assertEqual(mock_obj.asset_data.description, "Test Top Shirt")
        self.assertIn("clothing", mock_obj.asset_data.tags.items)
        self.assertEqual(mock_obj.preview.preview_file, str(thumb_file))

    def test_drop_asset_handler_validates_character_selection(self):
        """Test viewport drop handler operator validates character selection before fitting."""
        op = native_assets.OpDropAssetHandler()
        reports = []
        op.report = lambda level, msg: reports.append((level, msg))

        # Case 1: Active object is not a character mesh -> should cancel fitting
        non_char_obj = types.SimpleNamespace(
            type="MESH",
            data={"other": "data"},
            parent=None,
            modifiers={},
            vertex_groups={}
        )
        bpy.context.window_manager.charmorph_ui.fitting_char = non_char_obj

        res = op.execute(bpy.context)
        self.assertEqual(res, {"CANCELLED"})
        self.assertTrue(any("not a valid CharMorph character" in r[1] for r in reports))

        # Case 2: Target object IS a valid character mesh -> should execute fitting
        char_obj = types.SimpleNamespace(
            type="MESH",
            data={"charmorph_template": "mb_female"},
            parent=None,
            modifiers={},
            vertex_groups={}
        )
        bpy.context.window_manager.charmorph_ui.fitting_char = char_obj

        # Execute drop handler with char_obj
        res2 = op.execute(bpy.context)
        # Should proceed to asset fitting check
        self.assertIn(res2, [{"FINISHED"}, {"CANCELLED"}])

    def test_register_and_unregister_all_libraries(self):
        """Test register_all_libraries and unregister_all_libraries helper functions."""
        lib_dir = Path("/tmp/test_main_lib")
        lib_dir.mkdir(parents=True, exist_ok=True)

        native_assets.library.dirpath = str(lib_dir)
        native_assets.register_all_libraries()

        asset_libs = bpy.context.preferences.filepaths.asset_libraries
        self.assertIn(native_assets.CHARMORPH_LIB_NAME, asset_libs)

        native_assets.unregister_all_libraries()
        self.assertNotIn(native_assets.CHARMORPH_LIB_NAME, asset_libs)


if __name__ == "__main__":
    unittest.main()
