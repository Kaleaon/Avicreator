import pytest
import types

from CharMorphExpansion.lib.morpher import Morpher
from CharMorphExpansion import morphing


class DummyMorph:
    def __init__(self, name):
        self.name = name


class MockMeshData(dict):
    materials = []


class MockCore:
    def __init__(self, initial_values):
        data = MockMeshData()
        self.obj = types.SimpleNamespace(
            data=data, find_armature=lambda: None
        )
        self.char = types.SimpleNamespace(
            types={}, morphs_meta={}, armature={}
        )
        self.morphs_l1 = {}
        self.morphs_l2 = [DummyMorph(name) for name in initial_values.keys()]
        self.values = dict(initial_values)
        self.error = None
        self.alt_topo_buildable = False

    def check_vertex_count(self):
        return False

    def check_obj(self):
        return True

    def prop_get(self, name):
        return self.values.get(name, 0.0)

    def prop_set(self, name, value):
        self.values[name] = float(value)

    def update(self):
        pass


def create_mock_morpher(initial_values):
    core = MockCore(initial_values)
    obj = core.obj
    core.obj = None
    morpher = Morpher(core)
    core.obj = obj
    morpher.fitter = types.SimpleNamespace(refit_all=lambda: None)
    morpher.sj_calc = types.SimpleNamespace(recalc=lambda: None)
    morpher.update_rig = lambda: None
    morpher.materials = types.SimpleNamespace(
        apply=lambda *args, **kwargs: None
    )
    return morpher, core


def test_preset_mix_factor_interpolation_full():
    """mix_factor = 1.0 should apply 100% preset weight."""
    initial = {"morph_a": 0.2, "morph_b": 0.8}
    morpher, core = create_mock_morpher(initial)

    data = {"morphs": {"morph_a": 1.0, "morph_b": 0.0}}
    morpher.apply_morph_data(data, mix_factor=1.0)

    assert core.values["morph_a"] == pytest.approx(1.0)
    assert core.values["morph_b"] == pytest.approx(0.0)


def test_preset_mix_factor_interpolation_zero():
    """mix_factor = 0.0 should apply 0% preset weight (keep current)."""
    initial = {"morph_a": 0.2, "morph_b": 0.8}
    morpher, core = create_mock_morpher(initial)

    data = {"morphs": {"morph_a": 1.0, "morph_b": 0.0}}
    morpher.apply_morph_data(data, mix_factor=0.0)

    assert core.values["morph_a"] == pytest.approx(0.2)
    assert core.values["morph_b"] == pytest.approx(0.8)


def test_preset_mix_factor_interpolation_half():
    """mix_factor = 0.5 should apply 50% preset weight and 50% current."""
    initial = {"morph_a": 0.2, "morph_b": 0.8}
    morpher, core = create_mock_morpher(initial)

    data = {"morphs": {"morph_a": 1.0, "morph_b": 0.0}}
    morpher.apply_morph_data(data, mix_factor=0.5)

    assert core.values["morph_a"] == pytest.approx(0.6)
    assert core.values["morph_b"] == pytest.approx(0.4)


@pytest.mark.parametrize("mix_factor, expected_a, expected_b", [
    (0.25, 0.25, 0.75),
    (0.75, 0.75, 0.25),
    (0.1, 0.1, 0.9),
])
def test_preset_mix_factor_interpolation_intermediate(
    mix_factor, expected_a, expected_b
):
    """Verify linear weight interpolation across intermediate mix factors."""
    initial = {"morph_a": 0.0, "morph_b": 1.0}
    morpher, core = create_mock_morpher(initial)

    data = {"morphs": {"morph_a": 1.0, "morph_b": 0.0}}
    morpher.apply_morph_data(data, mix_factor=mix_factor)

    assert core.values["morph_a"] == pytest.approx(expected_a)
    assert core.values["morph_b"] == pytest.approx(expected_b)


def test_preset_mix_factor_clamping():
    """Out-of-bounds float mix factors should be clamped to [0.0, 1.0]."""
    initial = {"morph_a": 0.0, "morph_b": 1.0}
    data = {"morphs": {"morph_a": 1.0, "morph_b": 0.0}}

    # -0.5 should clamp to 0.0 (keep initial)
    morpher1, core1 = create_mock_morpher(initial)
    morpher1.apply_morph_data(data, mix_factor=-0.5)
    assert core1.values["morph_a"] == pytest.approx(0.0)
    assert core1.values["morph_b"] == pytest.approx(1.0)

    # 1.5 should clamp to 1.0 (apply full preset)
    morpher2, core2 = create_mock_morpher(initial)
    morpher2.apply_morph_data(data, mix_factor=1.5)
    assert core2.values["morph_a"] == pytest.approx(1.0)
    assert core2.values["morph_b"] == pytest.approx(0.0)


def test_preset_mix_factor_boolean_backward_compatibility():
    """True -> 0.5 (50/50 mix) and False -> 1.0 (100% preset)."""
    initial = {"morph_a": 0.2, "morph_b": 0.8}
    data = {"morphs": {"morph_a": 1.0, "morph_b": 0.0}}

    # True -> 0.5 factor
    morpher1, core1 = create_mock_morpher(initial)
    morpher1.apply_morph_data(data, mix_factor=True)
    assert core1.values["morph_a"] == pytest.approx(0.6)
    assert core1.values["morph_b"] == pytest.approx(0.4)

    # False -> 1.0 factor (full preset)
    morpher2, core2 = create_mock_morpher(initial)
    morpher2.apply_morph_data(data, mix_factor=False)
    assert core2.values["morph_a"] == pytest.approx(1.0)
    assert core2.values["morph_b"] == pytest.approx(0.0)


def test_uiprops_preset_mix_factor_definition():
    """Verify UIProps contains morph_preset_mix_factor property."""
    has_mix = hasattr(morphing.UIProps, "morph_preset_mix_factor") or (
        "morph_preset_mix_factor" in getattr(
            morphing.UIProps, "__annotations__", {}
        )
    )
    has_old = hasattr(morphing.UIProps, "morph_preset_mix") or (
        "morph_preset_mix" in getattr(
            morphing.UIProps, "__annotations__", {}
        )
    )
    assert has_mix
    assert not has_old
