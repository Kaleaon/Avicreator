# ##### BEGIN GPL LICENSE BLOCK #####
#
#  This program is free software; you can redistribute it and/or
#  modify it under the terms of the GNU General Public License
#  as published by the Free Software Foundation; either version 3
#  of the License, or (at your option) any later version.
#
#  This program is distributed in the hope that it will be useful,
#  but WITHOUT ANY WARRANTY; without even the implied warranty of
#  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#  GNU General Public License for more details.
#
#  You should have received a copy of the GNU General Public License
#  along with this program; if not, write to the Free Software Foundation,
#  Inc., 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301, USA.
#
# ##### END GPL LICENSE BLOCK #####

import os
import uuid
import logging
import bpy  # pylint: disable=import-error

try:
    from .lib.charlib import library, Asset
    from .assets import get_char, fitter_from_ctx, get_asset_conf
except (ImportError, ValueError):
    from lib.charlib import library, Asset
    from assets import get_char, fitter_from_ctx, get_asset_conf

logger = logging.getLogger(__name__)

CHARMORPH_LIB_NAME = "CharMorph Assets"
CHARMORPH_USER_LIB_NAME = "CharMorph User Library"

DEFAULT_CATEGORIES = [
    "CharMorph",
    "CharMorph/Clothing",
    "CharMorph/Clothing/Tops",
    "CharMorph/Clothing/Bottoms",
    "CharMorph/Clothing/FullBody",
    "CharMorph/Clothing/Underwear",
    "CharMorph/Hair",
    "CharMorph/Presets",
    "CharMorph/Accessories",
]


def generate_catalog_file(library_dir: str, custom_categories=None) -> dict[str, str]:
    """Generate or update blender_assets.cats.txt catalog mapping within library_dir."""
    if not library_dir or not os.path.isdir(library_dir):
        logger.warning("Catalog generation skipped: Directory %s does not exist", library_dir)
        return {}

    categories = list(DEFAULT_CATEGORIES)
    if custom_categories:
        for cat in custom_categories:
            if cat and cat not in categories:
                categories.append(cat)

    cat_file_path = os.path.join(library_dir, "blender_assets.cats.txt")
    catalog_map = {}

    lines = ["VERSION 1", ""]
    for cat_path in categories:
        cat_uuid = str(uuid.uuid5(uuid.NAMESPACE_DNS, cat_path))
        simple_name = cat_path.split("/")[-1]
        catalog_map[cat_path] = cat_uuid
        lines.append(f"{cat_uuid}:{cat_path}:{simple_name}")

    try:
        with open(cat_file_path, "w", encoding="utf-8") as f:
            f.write("\n".join(lines) + "\n")
        logger.info("Successfully generated asset catalog at %s", cat_file_path)
    except (OSError, IOError) as e:
        logger.error("Failed to write blender_assets.cats.txt at %s: %s", cat_file_path, e)

    return catalog_map


def register_asset_library(name: str, dir_path: str) -> bool:
    """Register an asset library path under user preferences preserving existing libraries."""
    if not dir_path or not os.path.isdir(dir_path):
        return False

    if not hasattr(bpy, "context") or not hasattr(bpy.context, "preferences"):
        return False

    try:
        asset_libraries = bpy.context.preferences.filepaths.asset_libraries
        if name in asset_libraries:
            asset_libraries[name].path = dir_path
        else:
            asset_libraries.new(name=name, path=dir_path)
        logger.info("Registered native asset library '%s' -> %s", name, dir_path)
        return True
    except Exception as e:
        logger.error("Failed to register asset library '%s': %s", name, e)
        return False


def unregister_asset_library(name: str) -> bool:
    """Non-destructively remove a specific CharMorph asset library from preferences."""
    if not hasattr(bpy, "context") or not hasattr(bpy.context, "preferences"):
        return False

    try:
        asset_libraries = bpy.context.preferences.filepaths.asset_libraries
        if name in asset_libraries:
            lib = asset_libraries[name]
            asset_libraries.remove(lib)
            logger.info("Unregistered native asset library '%s'", name)
            return True
    except Exception as e:
        logger.error("Failed to unregister asset library '%s': %s", name, e)
    return False


def update_user_asset_library(old_path: str, new_path: str):
    """Callback when user library directory property changes."""
    if old_path and old_path != new_path:
        unregister_asset_library(CHARMORPH_USER_LIB_NAME)

    if new_path and os.path.isdir(new_path):
        generate_catalog_file(new_path)
        register_asset_library(CHARMORPH_USER_LIB_NAME, new_path)


def register_all_libraries():
    """Register main and user asset libraries in Blender preferences."""
    if library.dirpath and os.path.isdir(library.dirpath):
        generate_catalog_file(library.dirpath)
        register_asset_library(CHARMORPH_LIB_NAME, library.dirpath)

    if hasattr(bpy.context, "window_manager") and hasattr(bpy.context.window_manager, "charmorph_ui"):
        user_dir = getattr(bpy.context.window_manager.charmorph_ui, "fitting_library_dir", "")
        if user_dir and os.path.isdir(user_dir):
            generate_catalog_file(user_dir)
            register_asset_library(CHARMORPH_USER_LIB_NAME, user_dir)


def unregister_all_libraries():
    """Unregister CharMorph asset libraries from user preferences."""
    unregister_asset_library(CHARMORPH_LIB_NAME)
    unregister_asset_library(CHARMORPH_USER_LIB_NAME)


def mark_as_native_asset(datablock, catalog_path: str = None, description: str = "", tags: tuple = (), thumb_path: str = None) -> bool:
    """Programmatically mark a mesh or object as a native asset and set metadata/thumbnail."""
    if datablock is None:
        return False

    if hasattr(datablock, "asset_mark"):
        datablock.asset_mark()
    else:
        # Fallback for mock objects in tests
        datablock.is_asset = True

    if catalog_path:
        cat_uuid = str(uuid.uuid5(uuid.NAMESPACE_DNS, catalog_path))
        if hasattr(datablock, "asset_data") and datablock.asset_data is not None:
            datablock.asset_data.catalog_id = cat_uuid

    if hasattr(datablock, "asset_data") and datablock.asset_data is not None:
        if description:
            datablock.asset_data.description = description
        if tags and hasattr(datablock.asset_data, "tags"):
            for tag in tags:
                if hasattr(datablock.asset_data.tags, "new"):
                    datablock.asset_data.tags.new(tag)

    if thumb_path and os.path.isfile(thumb_path):
        if hasattr(datablock, "preview") and hasattr(datablock.preview, "load_custom_preview_from_file"):
            datablock.preview.load_custom_preview_from_file(thumb_path)
        elif hasattr(bpy.ops, "ed") and hasattr(bpy.ops.ed, "lib_id_load_custom_preview"):
            try:
                bpy.ops.ed.lib_id_load_custom_preview(filepath=thumb_path)
            except Exception as e:
                logger.debug("Could not run lib_id_load_custom_preview: %s", e)

    return True


def is_character_mesh(obj) -> bool:
    """Validate whether an object is a valid CharMorph character mesh."""
    if not obj or not hasattr(obj, "data") or obj.data is None:
        return False
    if hasattr(obj, "type") and obj.type != "MESH":
        return False

    data = obj.data
    template = None
    if hasattr(data, "get"):
        template = data.get("charmorph_template")
    elif isinstance(data, dict):
        template = data.get("charmorph_template")

    if not template and hasattr(obj, "get"):
        template = obj.get("manuellab_id")

    if not template:
        return False

    char_info = library.char_by_name(str(template))
    return bool(char_info)


class OpDropAssetHandler(bpy.types.Operator):
    """Capture drag-and-drop operations on character objects and execute fitting routines."""
    bl_idname = "charmorph.drop_asset_handler"
    bl_label = "Drop Asset Handler"
    bl_description = "Fit dropped asset onto selected character mesh"
    bl_options = {"UNDO"}

    asset_name: bpy.props.StringProperty(
        name="Asset Name",
        description="Name of asset to fit",
        default=""
    )
    filepath: bpy.props.StringProperty(
        name="File Path",
        description="Path to .blend file of asset",
        default=""
    )

    @classmethod
    def poll(cls, context):
        return context.mode == "OBJECT"

    def execute(self, context):
        char_obj = get_char(context)
        if not char_obj or not is_character_mesh(char_obj):
            self.report({'ERROR'}, "Target object is not a valid CharMorph character mesh")
            return {"CANCELLED"}

        filepath = getattr(self, "filepath", "")
        asset_name = getattr(self, "asset_name", "")

        if filepath and os.path.isfile(filepath):
            name, _ = os.path.splitext(os.path.basename(filepath))
            if fitter_from_ctx(context).fit_import((Asset(name, filepath),)):
                self.report({'INFO'}, f"Successfully fitted asset from {filepath}")
                return {"FINISHED"}
            self.report({'ERROR'}, "Failed to fit dropped asset file")
            return {"CANCELLED"}

        if asset_name:
            asset_data = library.additional_assets.get(asset_name)
            if not asset_data and hasattr(char, "assets"):
                asset_data = char.assets.get(asset_name)
            if asset_data and fitter_from_ctx(context).fit_import((asset_data,)):
                self.report({'INFO'}, f"Successfully fitted asset '{asset_name}'")
                return {"FINISHED"}

        asset_data = get_asset_conf(context)
        if asset_data and fitter_from_ctx(context).fit_import((asset_data,)):
            return {"FINISHED"}

        self.report({'ERROR'}, "No valid asset selected for fitting")
        return {"CANCELLED"}


classes = [OpDropAssetHandler]


def register():
    register_all_libraries()


def unregister():
    unregister_all_libraries()
