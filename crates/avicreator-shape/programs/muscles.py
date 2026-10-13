"""Shared muscle recipes for bundled biped, quadruped and winged forms."""


def anchor(bone, at, offset=(0, 0, 0)):
    return {"attach": bone, "at": at, "offset": list(offset)}


def muscle(name, origin, insertion, radius, mirror=True):
    return {
        "kind": "muscle",
        "name": name,
        "mirror": mirror,
        "origin": origin,
        "insertion": insertion,
        "radius": radius,
        "tendon": 0.2,
        "blend": "0.024*(1-0.75*$muscle_definition)",
    }


def apply_muscles(program):
    """Replace static muscle masses; append after all endpoint bones exist.

    Idempotent so regenerating anthros retains the same anatomy and controls.
    Skeletons, skin features, feet, membranes and reference notes are retained.
    """
    name = program["name"]
    program["params"].setdefault("muscle_mass", {"value": 1.0, "min": 0.5, "max": 1.6})
    program["params"].setdefault("muscle_definition", {"value": 0.6, "min": 0.0, "max": 1.0})
    masses = {"glute", "trapezius", "neck_muscle", "deltoid", "biceps", "forearm", "thigh", "calf", "shin"}
    program["parts"] = [p for p in program["parts"] if p["kind"] != "muscle" and p["name"] not in masses]
    out = []
    if name in ("human", "canine", "scaly", "minotaur", "feline", "vulpine"):

        def add(n, a, b, r):
            out.append(muscle(n, a, b, f"{r}*$build*$muscle_mass"))

        add("glute", anchor("mPelvis", 0, (0.065, -0.02, -0.06)), anchor("mHipLeft", 0.3, (0, 0, -0.035)), 0.07)
        add("trapezius", anchor("mChest", 0.9, (0.035, 0, -0.04)), anchor("mCollarLeft", 0.8, (0, 0.02, -0.02)), 0.04)
        add("neck_muscle", anchor("mChest", 0.8, (0.04, 0, 0.03)), anchor("mNeck", 0.85, (0.035, 0, -0.02)), 0.017)
        add(
            "pectoralis",
            anchor("mChest", 0.45, (0.045, 0, 0.075)),
            anchor("mShoulderLeft", 0.08, (0, -0.025, 0.025)),
            0.04,
        )
        for j, level in enumerate((0.3, 0.65)):
            add(
                f"pectoralis_fan_{j}",
                anchor("mChest", level, (0.035, 0, 0.075)),
                anchor("mShoulderLeft", 0.12, (0, -0.02, 0.02)),
                0.027,
            )
        add(
            "latissimus",
            anchor("mTorso", 0.4, (0.075, 0, -0.07)),
            anchor("mShoulderLeft", 0.1, (0, -0.03, -0.025)),
            0.038,
        )
        add("oblique", anchor("mPelvis", 0.9, (0.095, 0, 0.055)), anchor("mChest", 0.25, (0.115, 0, 0.04)), 0.026)
        for j in range(3):
            add(
                f"rectus_abdominis_{j}",
                anchor("mTorso", j / 3, (0.035, 0, 0.09)),
                anchor("mTorso", (j + 0.85) / 3, (0.035, 0, 0.09)),
                0.029,
            )
        add("deltoid", anchor("mCollarLeft", 0.8, (0, 0.025, 0)), anchor("mShoulderLeft", 0.4, (0, 0.022, 0)), 0.052)
        add("biceps", anchor("mShoulderLeft", 0.15, (0, 0, 0.028)), anchor("mElbowLeft", 0.12, (0, 0, 0.018)), 0.042)
        add("triceps", anchor("mShoulderLeft", 0.1, (0, 0, -0.028)), anchor("mElbowLeft", 0.03, (0, 0, -0.018)), 0.037)
        add("forearm", anchor("mElbowLeft", 0.08, (0, 0.012, 0.012)), anchor("mWristLeft", 0.05, (0, 0, 0.008)), 0.036)
        add("thigh", anchor("mHipLeft", 0.08, (0, 0, 0.035)), anchor("mKneeLeft", 0.06, (0, 0, 0.02)), 0.07)
        add("hamstring", anchor("mHipLeft", 0.12, (0, 0, -0.04)), anchor("mKneeLeft", 0.05, (0, 0, -0.02)), 0.055)
        add(
            "calf" if name == "human" else "shin",
            anchor("mKneeLeft", 0.08, (0, 0, -0.026)),
            anchor("mAnkleLeft", 0.1, (0, 0, -0.012)),
            0.048,
        )
        if name in ("canine", "scaly", "feline", "vulpine"):
            for i in range(1, 4):
                add(
                    f"tail_muscle_{i}",
                    anchor(f"mTail{i}", 0.1, (0.02, 0, 0)),
                    anchor(f"mTail{i}", 0.9, (0.02, 0, 0)),
                    0.025 / i,
                )
        flex = {"mElbowLeft": {"bend": 70}, "mKneeLeft": {"bend": -35}}
    else:

        def add(n, a, b, r):
            out.append(muscle(n, a, b, f"{r}*$muscle_mass"))

        girth = "$girth" if name == "dragon" else "0.15"
        add(
            "epaxial",
            anchor("spine.0", 0.2, (f"{girth}*0.45", f"{girth}*0.6", 0)),
            anchor("spine.2", 0.8, (f"{girth}*0.35", f"{girth}*0.5", 0)),
            f"{girth}*0.3",
        )
        add(
            "neck_muscle",
            anchor("spine.2", 0.8, (f"{girth}*0.3", 0, 0)),
            anchor("neck.0" if name == "dragon" else "neck", 0.8, (f"{girth}*0.2", 0, 0)),
            f"{girth}*0.25",
        )
        for leg, size in (
            (("fore_leg", 0.10), ("hind_leg", 0.14))
            if name == "dragon"
            else (("front_leg", 0.045), ("back_leg", 0.055))
        ):
            add(
                leg + "_flexor",
                anchor(leg + ".0", 0.1, (0, 0, size * 0.5)),
                anchor(leg + ".1", 0.1, (0, 0, size * 0.3)),
                size,
            )
            add(
                leg + "_extensor",
                anchor(leg + ".0", 0.1, (0, 0, -size * 0.5)),
                anchor(leg + ".1", 0.05, (0, 0, -size * 0.3)),
                size * 0.8,
            )
            add(
                leg + "_calf",
                anchor(leg + ".1", 0.15, (0, 0, -size * 0.35)),
                anchor(leg + ".1", 0.9, (0, 0, -size * 0.15)),
                size * 0.65,
            )
        for i in range(3):
            add(
                f"tail_muscle_{i}",
                anchor(f"tail.{i}", 0.1, (f"{girth}*0.2", 0, 0)),
                anchor(f"tail.{i}", 0.9, (f"{girth}*0.2", 0, 0)),
                f"{girth}*{0.3 / (i + 1):.3f}",
            )
        if name == "dragon":
            add(
                "wing_pectoralis",
                anchor("spine.2", 0.2, ("$girth*0.5", 0, 0)),
                anchor("wing_arm.0", 0.5, (0, -0.02, 0)),
                "$girth*0.28",
            )
            add("wing_flexor", anchor("wing_arm.0", 0.1, (0, 0, 0.03)), anchor("wing_arm.1", 0.1, (0, 0, 0.02)), 0.065)
        flex = {
            ("fore_leg.1.L" if name == "dragon" else "front_leg.1.L"): {"bend": 35},
            ("hind_leg.1.L" if name == "dragon" else "back_leg.1.L"): {"bend": -30},
        }
        if name == "dragon":
            flex["wing_arm.1.L"] = {"bend": 30}
    program["parts"].extend(out)
    program.setdefault("poses", {})["muscle_flex"] = flex
    return program
