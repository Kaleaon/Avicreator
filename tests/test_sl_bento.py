# Unit tests for Second Life Bento armature definitions and configurations

import unittest
import os
import sys

# Ensure CharMorph root directory is in sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from lib import sl_bento, utils, charlib
from lib.charlib import Character, DataDir


class TestSLBento(unittest.TestCase):

    def test_sl_bento_bone_hierarchy(self):
        """Verify standard mBone hierarchy in Second Life Bento armature definition."""
        bones = sl_bento.SL_BENTO_BONES
        self.assertIn("mPelvis", bones)
        self.assertIsNone(bones["mPelvis"]["parent"])

        # Check spine hierarchy
        self.assertEqual(bones["mTorso"]["parent"], "mPelvis")
        self.assertEqual(bones["mChest"]["parent"], "mTorso")
        self.assertEqual(bones["mNeck"]["parent"], "mChest")
        self.assertEqual(bones["mHead"]["parent"], "mNeck")

        # Check limb hierarchies
        self.assertEqual(bones["mHipLeft"]["parent"], "mPelvis")
        self.assertEqual(bones["mKneeLeft"]["parent"], "mHipLeft")
        self.assertEqual(bones["mAnkleLeft"]["parent"], "mKneeLeft")
        self.assertEqual(bones["mFootLeft"]["parent"], "mAnkleLeft")
        self.assertEqual(bones["mToeLeft"]["parent"], "mFootLeft")

        self.assertEqual(bones["mCollarLeft"]["parent"], "mChest")
        self.assertEqual(bones["mShoulderLeft"]["parent"], "mCollarLeft")
        self.assertEqual(bones["mElbowLeft"]["parent"], "mShoulderLeft")
        self.assertEqual(bones["mWristLeft"]["parent"], "mElbowLeft")
        self.assertEqual(bones["mHandLeft"]["parent"], "mWristLeft")

        # Check Bento finger bones
        self.assertEqual(bones["mHandThumb1Left"]["parent"], "mWristLeft")
        self.assertEqual(bones["mHandIndex1Left"]["parent"], "mWristLeft")
        self.assertEqual(bones["mHandMiddle1Left"]["parent"], "mWristLeft")
        self.assertEqual(bones["mHandRing1Left"]["parent"], "mWristLeft")
        self.assertEqual(bones["mHandPinky1Left"]["parent"], "mWristLeft")

    def test_sl_collision_volumes(self):
        """Verify collision volume bones (cBones) in SL Bento definition."""
        cbones = sl_bento.SL_COLLISION_VOLUMES
        self.assertIn("PELVIS", cbones)
        self.assertEqual(cbones["PELVIS"]["parent"], "mPelvis")
        self.assertEqual(cbones["TORSO"]["parent"], "mTorso")
        self.assertEqual(cbones["CHEST"]["parent"], "mChest")
        self.assertEqual(cbones["NECK"]["parent"], "mNeck")
        self.assertEqual(cbones["HEAD"]["parent"], "mHead")

        self.assertEqual(cbones["HIP_LEFT"]["parent"], "mHipLeft")
        self.assertEqual(cbones["KNEELFT"]["parent"], "mKneeLeft")
        self.assertEqual(cbones["ANKLELFT"]["parent"], "mAnkleLeft")
        self.assertEqual(cbones["FOOTLFT"]["parent"], "mFootLeft")

        self.assertEqual(cbones["SHOULDERLFT"]["parent"], "mShoulderLeft")
        self.assertEqual(cbones["ELBOWLFT"]["parent"], "mElbowLeft")
        self.assertEqual(cbones["WRISTLFT"]["parent"], "mWristLeft")

        # Confirm deform flag is False for collision volumes
        self.assertFalse(cbones["PELVIS"]["deform"])

    def test_charmorph_to_sl_weight_mapping(self):
        """Verify vertex group mapping table between CharMorph groups and SL mBones."""
        weight_map = sl_bento.CHARMORPH_TO_SL_WEIGHT_MAP
        self.assertEqual(weight_map["pelvis"], "mPelvis")
        self.assertEqual(weight_map["chest"], "mChest")
        self.assertEqual(weight_map["neck"], "mNeck")
        self.assertEqual(weight_map["head"], "mHead")
        self.assertEqual(weight_map["thigh.L"], "mHipLeft")
        self.assertEqual(weight_map["shin.L"], "mKneeLeft")
        self.assertEqual(weight_map["foot.L"], "mAnkleLeft")

    def test_is_sl_armature_validation(self):
        """Verify is_sl_armature validation logic."""
        sl_rig = {"bones": {"mPelvis": {}, "mChest": {}, "mHead": {}}}
        non_sl_rig = {"bones": {"root": {}, "spine": {}, "head": {}}}

        self.assertTrue(sl_bento.is_sl_armature(sl_rig))
        self.assertFalse(sl_bento.is_sl_armature(non_sl_rig))

    def test_character_configs_contain_sl_bento(self):
        """Verify character config files offer Second Life Bento armature."""
        data_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "data"))
        chars_dir = os.path.join(data_dir, "characters")

        for char_name in ["mb_female", "mb_male", "antonia", "reom"]:
            config_path = os.path.join(chars_dir, char_name, "config.yaml")
            self.assertTrue(os.path.exists(config_path), f"Missing config for {char_name}")

            char = Character(char_name, DataDir(data_dir))
            armatures = char.armature
            self.assertIn("sl_bento", armatures, f"sl_bento missing in {char_name} config")
            self.assertEqual(armatures["sl_bento"].title, "Second Life Bento")
            self.assertEqual(armatures["sl_bento"].type, "sl_bento")

    def test_non_sl_armatures_unaffected(self):
        """Verify non-SL armatures remain intact."""
        data_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "data"))
        config_path = os.path.join(data_dir, "characters", "mb_female", "config.yaml")

        with open(config_path, "r", encoding="utf-8") as f:
            conf = utils.load_yaml(f)

        armatures = conf.get("armature", {})
        self.assertIn("tweaked", armatures)
        self.assertIn("original", armatures)
        self.assertIn("gaming", armatures)


if __name__ == "__main__":
    unittest.main()
