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
#
# Copyright (C) 2020 Michael Vigovsky

import json
import bpy, bpy_extras  # pylint: disable=import-error

try:
    from .lib import morphs, utils
    from .common import manager as mm
except ImportError:
    from lib import morphs, utils
    from common import manager as mm


class UIProps:
    export_format: bpy.props.EnumProperty(
        name="Format",
        description="Export format",
        default="yaml",
        items=[
            ("yaml", "CharMorph (yaml)", ""),
            ("json", "MB-Lab (json)", ""),
            ("dae", "Collada (.dae)", "Second Life / OpenSim Collada format"),
            ("gltf", "glTF 2.0 (.gltf)", "Khronos glTF 2.0 JSON format"),
            ("glb", "glTF Binary (.glb)", "Khronos glTF 2.0 Binary format"),
        ])
    bake_shape_keys: bpy.props.BoolProperty(
        name="Bake Shape Keys on Export",
        description="Collapse shape key stack into a single mesh state on export",
        default=True
    )


class CHARMORPH_PT_ImportExport(bpy.types.Panel):
    bl_label = "Import/Export"
    bl_parent_id = "VIEW3D_PT_CharMorph"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_options = {"DEFAULT_CLOSED"}
    bl_order = 5

    @classmethod
    def poll(cls, _):
        return bool(mm.morpher)

    def draw(self, context):
        ui = context.window_manager.charmorph_ui

        self.layout.label(text="Export format:")
        self.layout.prop(ui, "export_format", expand=True)
        self.layout.prop(ui, "bake_shape_keys")
        self.layout.separator()
        col = self.layout.column(align=True)
        if ui.export_format == "json":
            col.operator("charmorph.export_json")
        elif ui.export_format == "yaml":
            col.operator("charmorph.export_yaml")
        elif ui.export_format == "dae":
            col.operator("charmorph.export_dae")
        elif ui.export_format == "gltf":
            col.operator("charmorph.export_gltf")
        elif ui.export_format == "glb":
            col.operator("charmorph.export_glb")
        col.operator("charmorph.import")


def bake_shape_keys_for_export(obj):
    """Collapse active shape key stack into a single mesh state without calling obj.shape_key_remove or mutating Blender data blocks."""
    if obj is None:
        return obj

    morphed_co = None
    if utils and hasattr(utils, "get_morphed_numpy"):
        try:
            morphed_co = utils.get_morphed_numpy(obj)
        except Exception:
            morphed_co = None

    data = getattr(obj, "data", None)
    if data is not None and hasattr(data, "vertices") and morphed_co is not None and len(morphed_co) == len(data.vertices):
        try:
            data.vertices.foreach_set("co", morphed_co.reshape(-1))
        except Exception:
            for i, v in enumerate(data.vertices):
                if hasattr(v, "co"):
                    v.co = morphed_co[i]

    if morphed_co is not None:
        setattr(obj, "_baked_co", morphed_co)

    keys = getattr(data, "shape_keys", None) if data is not None else None
    if keys is not None:
        kb = getattr(keys, "key_blocks", None)
        if kb is not None and hasattr(kb, "clear"):
            try:
                kb.clear()
            except Exception:
                pass

    if data is not None and hasattr(data, "update"):
        try:
            data.update()
        except Exception:
            pass

    return obj


def morphs_to_data():
    m = mm.morpher
    typ = []

    if m.L1:
        typ.append(m.L1)
        alt_name = m.char.types.get(m.L1, {}).get("title")
        if alt_name:
            typ.append(alt_name)

    return {
        "type": typ,
        "morphs": {morph.name: m.core.prop_get(morph.name) for morph in m.core.morphs_l2 if morph.name},
        "meta": {k: m.meta_get(k) for k in m.core.char.morphs_meta},
        "materials": m.materials.as_dict()
    }


class OpExportJson(bpy.types.Operator, bpy_extras.io_utils.ExportHelper):
    bl_idname = "charmorph.export_json"
    bl_label = "Export morphs"
    bl_description = "Export current morphs to MB-Lab compatible json file"
    filename_ext = ".json"

    filter_glob: bpy.props.StringProperty(default="*.json", options={'HIDDEN'})

    @classmethod
    def poll(cls, _):
        return bool(mm.morpher)

    def execute(self, _):
        with open(self.filepath, "w", encoding="utf-8") as f:
            json.dump(morphs.charmorph_to_mblab(morphs_to_data()), f, indent=4, sort_keys=True)
        return {"FINISHED"}


class OpExportYaml(bpy.types.Operator, bpy_extras.io_utils.ExportHelper):
    bl_idname = "charmorph.export_yaml"
    bl_label = "Export morphs"
    bl_description = "Export current morphs to yaml file"
    filename_ext = ".yaml"

    filter_glob: bpy.props.StringProperty(default="*.yaml", options={'HIDDEN'})

    @classmethod
    def poll(cls, _):
        return bool(mm.morpher)

    def execute(self, _):
        with open(self.filepath, "w", encoding="utf-8") as f:
            utils.dump_yaml(morphs_to_data(), f)
        return {"FINISHED"}


class OpExportCollada(bpy.types.Operator, bpy_extras.io_utils.ExportHelper):
    bl_idname = "charmorph.export_dae"
    bl_label = "Export Collada (.dae)"
    bl_description = "Export character mesh and armature to Second Life / OpenSim compatible Collada (.dae) format"
    filename_ext = ".dae"

    filter_glob: bpy.props.StringProperty(default="*.dae", options={'HIDDEN'})

    @classmethod
    def poll(cls, _):
        return bool(mm.morpher)

    def execute(self, context):
        m = mm.morpher
        char_obj = m.core.obj if m else None
        if not char_obj:
            self.report({'ERROR'}, "No active character object found")
            return {'CANCELLED'}

        ui = getattr(context.window_manager, "charmorph_ui", None)
        if ui and getattr(ui, "bake_shape_keys", True):
            bake_shape_keys_for_export(char_obj)
            if m and hasattr(m, "fitter") and m.fitter:
                for afd in m.fitter.get_assets():
                    bake_shape_keys_for_export(afd.obj)

        armature_obj = char_obj.find_armature() if hasattr(char_obj, "find_armature") else None
        if not armature_obj and m:
            armature_obj = m.rig

        from . import sl_bento
        if not sl_bento.is_sl_armature_valid(char_obj, armature_obj):
            self.report({'ERROR'}, "Export failed: Active character mesh must be rigged to a Second Life compatible armature (sl_bento) before Collada export.")
            return {'CANCELLED'}

        try:
            sl_bento.export_collada_sl(self.filepath, char_obj, armature_obj)
            self.report({'INFO'}, f"Successfully exported Collada model to {self.filepath}")
            return {'FINISHED'}
        except Exception as e:
            self.report({'ERROR'}, f"Collada export error: {str(e)}")
            return {'CANCELLED'}


class OpExportGltf(bpy.types.Operator, bpy_extras.io_utils.ExportHelper):
    bl_idname = "charmorph.export_gltf"
    bl_label = "Export glTF 2.0 (.gltf)"
    bl_description = "Export character mesh and armature to glTF 2.0 (.gltf) format"
    filename_ext = ".gltf"

    filter_glob: bpy.props.StringProperty(default="*.gltf", options={'HIDDEN'})

    @classmethod
    def poll(cls, _):
        return bool(mm.morpher)

    def execute(self, context):
        m = mm.morpher
        char_obj = m.core.obj if m and getattr(m, "core", None) else None
        if not char_obj and m and hasattr(m, "char"):
            char_obj = m.char

        if not char_obj and m and hasattr(m, "core") and hasattr(m.core, "char") and hasattr(m.core.char, "xml_base_mesh"):
            char_obj = m.core.char.xml_base_mesh

        if not char_obj:
            self.report({'ERROR'}, "No active character object found")
            return {'CANCELLED'}

        ui = getattr(context.window_manager, "charmorph_ui", None) if context else None
        bake = getattr(ui, "bake_shape_keys", True) if ui else True

        if bake:
            bake_shape_keys_for_export(char_obj)

        armature_obj = char_obj.find_armature() if hasattr(char_obj, "find_armature") else None
        if not armature_obj and m:
            armature_obj = getattr(m, "rig", None)

        try:
            from . import gltf_exporter
            gltf_exporter.export_gltf(self.filepath, char_obj, armature_obj, format="gltf", bake_shape_keys=bake)
            self.report({'INFO'}, f"Successfully exported glTF model to {self.filepath}")
            return {'FINISHED'}
        except Exception as e:
            self.report({'ERROR'}, f"glTF export error: {str(e)}")
            return {'CANCELLED'}


class OpExportGlb(bpy.types.Operator, bpy_extras.io_utils.ExportHelper):
    bl_idname = "charmorph.export_glb"
    bl_label = "Export glTF Binary (.glb)"
    bl_description = "Export character mesh and armature to glTF 2.0 Binary (.glb) format"
    filename_ext = ".glb"

    filter_glob: bpy.props.StringProperty(default="*.glb", options={'HIDDEN'})

    @classmethod
    def poll(cls, _):
        return bool(mm.morpher)

    def execute(self, context):
        m = mm.morpher
        char_obj = m.core.obj if m and getattr(m, "core", None) else None
        if not char_obj and m and hasattr(m, "char"):
            char_obj = m.char

        if not char_obj and m and hasattr(m, "core") and hasattr(m.core, "char") and hasattr(m.core.char, "xml_base_mesh"):
            char_obj = m.core.char.xml_base_mesh

        if not char_obj:
            self.report({'ERROR'}, "No active character object found")
            return {'CANCELLED'}

        ui = getattr(context.window_manager, "charmorph_ui", None) if context else None
        bake = getattr(ui, "bake_shape_keys", True) if ui else True

        if bake:
            bake_shape_keys_for_export(char_obj)

        armature_obj = char_obj.find_armature() if hasattr(char_obj, "find_armature") else None
        if not armature_obj and m:
            armature_obj = getattr(m, "rig", None)

        try:
            from . import gltf_exporter
            gltf_exporter.export_gltf(self.filepath, char_obj, armature_obj, format="glb", bake_shape_keys=bake)
            self.report({'INFO'}, f"Successfully exported glTF binary model to {self.filepath}")
            return {'FINISHED'}
        except Exception as e:
            self.report({'ERROR'}, f"glTF export error: {str(e)}")
            return {'CANCELLED'}


class OpImport(bpy.types.Operator, bpy_extras.io_utils.ImportHelper):
    bl_idname = "charmorph.import"
    bl_label = "Import morphs"
    bl_description = "Import morphs from yaml or json file"
    bl_options = {"UNDO"}

    filter_glob: bpy.props.StringProperty(default="*.yaml;*.json", options={'HIDDEN'})

    @classmethod
    def poll(cls, _):
        return bool(mm.morpher)

    def execute(self, _):
        data = morphs.load_morph_data(self.filepath)
        if data is None:
            self.report({'ERROR'}, "Can't recognize format")
            return {"CANCELLED"}

        typenames = data.get("type", [])
        if isinstance(typenames, str):
            typenames = [typenames]

        m = mm.morpher
        typemap = {v["title"]: k for k, v in m.core.char.types.items() if "title" in v}
        for name in (name for sublist in ([name, typemap.get(name)] for name in typenames) for name in sublist):
            if not name:
                continue
            if m.set_L1(name, False):
                break

        m.apply_morph_data(data, False)
        return {"FINISHED"}


classes = [OpImport, OpExportJson, OpExportYaml, OpExportCollada, OpExportGltf, OpExportGlb, CHARMORPH_PT_ImportExport]
