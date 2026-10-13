"""Generate additional forms from shared anatomy; run after make_anthro.py."""

from copy import deepcopy
import json
from pathlib import Path
from format_program import dump
from muscles import apply_muscles, anchor, muscle

ROOT = Path(__file__).resolve().parent


def load(form_name):
    return json.loads((ROOT / (form_name + ".json")).read_text(encoding="utf-8"))


def write(form):
    (ROOT / (form["name"] + ".json")).write_text(dump(form) + "\n", encoding="utf-8")


def find(parts, part_name):
    return next(part for part in parts if part["name"] == part_name)


for name, muzzle, ear in (("feline", 0.04, 0.075), ("vulpine", 0.075, 0.145)):
    p = load("canine")
    p["name"] = name
    p["params"]["muzzle_length"]["value"] = muzzle
    p["params"]["ear_length"]["value"] = ear
    p["params"]["build"]["value"] = 0.9 if name == "feline" else 0.85
    p["params"]["tail_length"]["value"] = 0.85 if name == "feline" else 0.7
    head = find(p["parts"], "head")["parts"]
    find(head, "cheek_ruff")["radii"] = [0.04, 0.035, 0.045] if name == "vulpine" else [0.03, 0.03, 0.035]
    find(head, "cranium")["radii"] = [0.07, 0.075, 0.073]
    tail = find(p["parts"], "tail")
    tail["radius"] = [0.045, 0.07, 0.09, 0.07, 0.04, 0.01] if name == "vulpine" else [0.035, 0.028, 0.02, 0.012]
    p["notes"] = (
        f"Original {name} anthropomorphic anatomy; feline/vulpine head, claws and articulated tail. See docs/muscle_modelling.md for reference review."
    )
    apply_muscles(p)
    write(p)

p = load("canine")
p["name"] = "minotaur"
p["params"]["build"]["value"] = 1.3
p["params"]["muscle_mass"]["value"] = 1.25
p["params"]["head_size"]["value"] = 1.2
p["params"]["muzzle_length"]["value"] = 0.075
p["params"]["ear_length"]["value"] = 0.09
p["params"]["horn_length"] = {"value": 0.18, "min": 0.08, "max": 0.35}
p["params"]["horn_curl"] = {"value": 65, "min": 0, "max": 110}
head = find(p["parts"], "head")["parts"]
find(head, "cranium")["radii"] = [0.095, 0.09, 0.085]
find(head, "muzzle")["radius"] = [0.06, 0.048, 0.045]
find(head, "nose_pad")["radii"] = [0.042, 0.025, 0.025]
head.append(
    {
        "kind": "chain",
        "name": "bull_horn",
        "mirror": True,
        "attach": "mHead",
        "at": 0,
        "offset": [0.075, 0.145, -0.02],
        "dir": [1, 0.12, -0.25],
        "length": "$horn_length",
        "segments": 5,
        "curl": "$horn_curl",
        "bend_axis": [0, 0, 1],
        "radius": [0.032, 0.002],
        "limits": {"bend": [0, 0], "side": [0, 0], "twist": [0, 0]},
    }
)
p["parts"] = [x for x in p["parts"] if x["name"] not in ("chest_fluff", "foot_pad") and not x["name"].endswith("_claw")]
p["parts"].append(
    {
        "kind": "blob",
        "name": "cloven_hoof",
        "mirror": True,
        "attach": "mFootLeft",
        "at": 0.4,
        "offset": [0, -0.025, 0],
        "radii": [0.055, 0.04, 0.065],
        "bone": False,
        "blend": 0.012,
    }
)
p["parts"].append(
    {
        "kind": "blob",
        "name": "hoof_cleft",
        "mirror": True,
        "op": "subtract",
        "attach": "mFootLeft",
        "at": 0.4,
        "offset": [0, -0.03, 0.05],
        "radii": [0.004, 0.05, 0.045],
        "blend": 0.001,
    }
)
# A bovine tail is slender, with a distal tuft rather than canine fur volume.
find(p["parts"], "tail")["radius"] = [0.018, 0.012, 0.008]
p["notes"] = (
    "Original bull-headed biped with horn, hoof and neck/shoulder masses informed by bull profile photographs; fantasy anatomy is an artistic interpretation."
)
apply_muscles(p)
write(p)

# Centaur: equine barrel, four independent legs, plus the shared humanoid upper body.
human = load("human")
p = deepcopy(human)
p["name"] = "centaur"
p["skeleton"] = "custom"
p["params"].pop("hip_height")
p["params"]["horse_length"] = {"value": 1.4, "min": 1.1, "max": 1.9}
p["params"]["horse_height"] = {"value": 0.95, "min": 0.8, "max": 1.2}
p["params"]["horse_girth"] = {"value": 0.29, "min": 0.24, "max": 0.38}
p["params"]["tail_length"] = {"value": 0.72, "min": 0.4, "max": 1.0}
exclude = {"leg", "thigh", "hamstring", "calf", "foot", "heel", "glute"}
upper = [x for x in p["parts"] if x["name"] not in exclude]
spine = find(upper, "spine")
spine.pop("from")
spine.update(attach="equine_spine.2", at=0.85, offset=[0, 0.13, 0])
lower = [
    {
        "kind": "chain",
        "name": "equine_spine",
        "from": [0, "$horse_height+0.1", "-$horse_length*0.5"],
        "dir": [0, 0, 1],
        "length": "$horse_length",
        "segments": 3,
        "radius": ["$horse_girth*0.82", "$horse_girth", "$horse_girth*0.8"],
    },
    {
        "kind": "blob",
        "name": "horse_barrel",
        "attach": "equine_spine.1",
        "at": 0.5,
        "offset": [0, "-$horse_girth*0.22", 0],
        "radii": ["$horse_girth", "$horse_girth*1.1", "$horse_length*0.38"],
        "bone": False,
    },
    {
        "kind": "chain",
        "name": "horse_tail",
        "attach": "equine_spine.0",
        "at": 0,
        "dir": [0, -1, -0.25],
        "length": "$tail_length",
        "segments": 5,
        "radius": [0.04, 0.06, 0.035, 0.008],
    },
]
leg_recipes = (
    (
        "horse_front",
        2,
        0.8,
        [
            [0, "-$horse_height*0.4", -0.03],
            [0.01, "-$horse_height*0.73", 0.04],
            [0.01, "-$horse_height*0.94", 0.02],
            [0.01, "-$horse_height", 0.065],
        ],
        [0.085, 0.055, 0.028, 0.025, 0.04],
    ),
    (
        "horse_hind",
        0,
        0.2,
        [
            [0.015, "-$horse_height*0.36", 0.13],
            [0.02, "-$horse_height*0.7", -0.08],
            [0.02, "-$horse_height*0.94", -0.04],
            [0.02, "-$horse_height", 0.02],
        ],
        [0.12, 0.08, 0.035, 0.027, 0.045],
    ),
)
for name, seg, at, points, radii in leg_recipes:
    lower.append(
        {
            "kind": "chain",
            "name": name,
            "mirror": True,
            "attach": f"equine_spine.{seg}",
            "at": at,
            "offset": ["$horse_girth*0.65", -0.05, 0],
            "points": points,
            "radius": radii,
        }
    )
    lower.append(
        {
            "kind": "blob",
            "name": name + "_hoof",
            "mirror": True,
            "attach": name,
            "offset": [0, -0.005, 0.018],
            "radii": [0.047, 0.035, 0.06],
            "bone": False,
            "blend": 0.008,
        }
    )
p["parts"] = lower + upper
for name, bone, radius in [("equine_shoulder", "horse_front", 0.095), ("equine_glute", "horse_hind", 0.13)]:
    p["parts"].append(
        muscle(
            name,
            anchor(bone + ".0", 0.04, (0.03, 0, -0.02)),
            anchor(bone + ".1", 0.12, (0.01, 0, 0)),
            f"{radius}*$muscle_mass",
        )
    )
    p["parts"].append(
        muscle(
            name + "_antagonist",
            anchor(bone + ".0", 0.1, (-0.02, 0, 0.02)),
            anchor(bone + ".1", 0.07, (0, 0, 0.02)),
            f"{radius*0.7}*$muscle_mass",
        )
    )
    p["parts"].append(
        muscle(
            name + "_tendon",
            anchor(bone + ".1", 0.2, (0, 0, -0.025)),
            anchor(bone + ".2", 0.75, (0, 0, -0.012)),
            "0.025*$muscle_mass",
        )
    )
p["parts"].append(
    muscle(
        "equine_longissimus",
        anchor("equine_spine.0", 0.15, (0.1, 0.16, 0)),
        anchor("equine_spine.2", 0.85, (0.1, 0.16, 0)),
        "0.075*$muscle_mass",
    )
)
p["poses"] = {
    "muscle_flex": {"mElbowLeft": {"bend": 70}, "horse_front.1.L": {"bend": 25}, "horse_hind.1.L": {"bend": -25}}
}
p["notes"] = (
    "Original centaur: equine proportions and distal tendon taper checked against a standing horse photograph. Custom six-limb rig; GLB export. Fantasy torso junction is an artistic design."
)
write(p)
