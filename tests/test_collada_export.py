# Unit tests for Collada (.dae) export operator and SL validator

import unittest
import os
import sys
import tempfile
import xml.etree.ElementTree as ET

# Ensure CharMorph root directory is in sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from lib import sl_bento


class MockMeshObject:
    def __init__(self, name="cm_mesh", armature=None):
        self.name = name
        self.armature = armature
        self.parent = armature

    def find_armature(self):
        return self.armature


class MockArmatureObject:
    def __init__(self, rig_type="sl_bento", bones=None):
        self.data = {"charmorph_rig_type": rig_type}
        self.bones = bones or {"mPelvis": {}, "mChest": {}, "mHead": {}}


class TestColladaExport(unittest.TestCase):

    def test_is_sl_armature_valid(self):
        """Verify armature validation logic for Collada export."""
        sl_arm = MockArmatureObject(rig_type="sl_bento")
        sl_mesh = MockMeshObject(name="avatar_mesh", armature=sl_arm)

        rigify_arm = MockArmatureObject(rig_type="rigify", bones={"root": {}, "spine": {}})
        rigify_mesh = MockMeshObject(name="avatar_mesh", armature=rigify_arm)

        unrigged_mesh = MockMeshObject(name="unrigged", armature=None)

        self.assertTrue(sl_bento.is_sl_armature_valid(sl_mesh, sl_arm))
        self.assertFalse(sl_bento.is_sl_armature_valid(rigify_mesh, rigify_arm))
        self.assertFalse(sl_bento.is_sl_armature_valid(unrigged_mesh, None))

    def test_export_collada_sl_invalid_armature_raises(self):
        """Verify export raises ValueError if mesh is not rigged to an SL armature."""
        rigify_arm = MockArmatureObject(rig_type="rigify", bones={"root": {}})
        rigify_mesh = MockMeshObject(name="avatar_mesh", armature=rigify_arm)

        with tempfile.TemporaryDirectory() as tmpdir:
            out_file = os.path.join(tmpdir, "test_export.dae")
            with self.assertRaises(ValueError) as ctx:
                sl_bento.export_collada_sl(out_file, rigify_mesh, rigify_arm)

            self.assertIn("Second Life compatible armature", str(ctx.exception))

    def test_export_collada_sl_success(self):
        """Verify Collada export emits valid .dae file with Z_UP and SL Bento structure."""
        sl_arm = MockArmatureObject(rig_type="sl_bento")
        sl_mesh = MockMeshObject(name="test_avatar", armature=sl_arm)

        with tempfile.TemporaryDirectory() as tmpdir:
            out_file = os.path.join(tmpdir, "test_avatar.dae")
            sl_bento.export_collada_sl(out_file, sl_mesh, sl_arm)

            self.assertTrue(os.path.exists(out_file))

            # Parse exported XML
            tree = ET.parse(out_file)
            root = tree.getroot()

            # Check Collada namespace and tag
            self.assertTrue(root.tag.endswith("COLLADA"))

            # Check Z_UP orientation
            up_axis = root.find(".//{*}up_axis")
            self.assertIsNotNone(up_axis)
            self.assertEqual(up_axis.text, "Z_UP")

            # Check unit meter = 1
            unit = root.find(".//{*}unit")
            self.assertIsNotNone(unit)
            self.assertEqual(unit.attrib.get("meter"), "1.0")

            # Check skeleton mPelvis reference
            skel = root.find(".//{*}skeleton")
            self.assertIsNotNone(skel)
            self.assertEqual(skel.text, "#mPelvis")

            # Check bind shape matrix
            bind_shape = root.find(".//{*}bind_shape_matrix")
            self.assertIsNotNone(bind_shape)
            self.assertIn("1 0 0 0", bind_shape.text)


if __name__ == "__main__":
    unittest.main()
