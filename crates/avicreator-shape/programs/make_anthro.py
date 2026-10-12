"""Generates furry (canine) and scaly (reptile) anthro programs from the human body."""
import json, sys

PROGRAMS = __import__("os").path.dirname(__import__("os").path.abspath(__file__)) + "/"
human = json.load(open(PROGRAMS + 'human.json'))
by_name = {p['name']: p for p in human['parts']}

def take(*names):
    return [json.loads(json.dumps(by_name[n])) for n in names]

def digitigrade(thick):
    return [
        {"kind": "chain", "name": "leg", "mirror": True, "attach": "mPelvis", "at": 0, "offset": ["0.095*$build", -0.055, 0],
         "points": [[0.005, "-($hip_height-0.06)*0.36", 0.13], [0.01, "-($hip_height-0.06)*0.72", -0.07],
                    [0.012, "-$hip_height+0.075", 0.07], [0.012, "-$hip_height+0.07", 0.13]],
         "bones": ["mHipLeft", "mKneeLeft", "mAnkleLeft", "mFootLeft"],
         "radius": [f"{0.082*thick:.3f}*$build", f"{0.056*thick:.3f}*$build", 0.036, 0.03, 0.022],
         "limits": {"bend": [-120, 40], "side": [-45, 45]}},
        {"kind": "blob", "name": "thigh", "mirror": True, "attach": "mHipLeft", "at": 0.45, "radii": [f"{0.085*thick:.3f}*$build", 0.15, f"{0.1*thick:.3f}*$build"], "bone": False},
        {"kind": "blob", "name": "shin", "mirror": True, "attach": "mKneeLeft", "at": 0.4, "offset": [0, 0, -0.01], "radii": [f"{0.052*thick:.3f}*$build", 0.12, f"{0.058*thick:.3f}*$build"], "bone": False},
        {"kind": "blob", "name": "hock", "mirror": True, "attach": "mAnkleLeft", "at": 0.1, "radii": [0.032, 0.04, 0.032], "bone": False, "blend": 0.02},
        {"kind": "blob", "name": "foot_pad", "mirror": True, "attach": "mFootLeft", "at": 0.3, "offset": [0, -0.012, 0], "radii": [0.048, 0.026, 0.065], "bone": False, "blend": 0.02},
    ]

def claws(fingers, length, radius):
    out = []
    for f in fingers:
        out.append({
            "kind": "chain", "name": f + "_claw", "mirror": True, "rigid": True, "attach": f, "at": 0.85,
            "dir": [1, -0.6, 0], "length": length, "radius": [radius, 0.0008], "blend": 0.002
        })
    return out

def hand(fingers, length_scale, thick, curl):
    """Fingers of the human hand, shortened and thickened."""
    out = []
    for p in take(*fingers):
        p['length'] = f"{length_scale}*{p['length']}"
        p['radius'] = [f"{thick}*{r}" if isinstance(r, str) else r * thick for r in p['radius']]
        p['curl'] = curl
        out.append(p)
    return out

# ---------------------------------------------------------------- canine head
canine_head = {"kind": "scope", "name": "head", "scale": "$head_size", "parts": [
    {"kind": "blob", "name": "cranium", "attach": "mNeck", "offset": [0, 0.1, -0.012], "radii": [0.07, 0.08, 0.084], "bone": "mHead", "limits": {"bend": [-40, 40]}},
    {"kind": "blob", "name": "brow_mass", "attach": "mHead", "at": 0, "offset": [0, 0.108, 0.045], "radii": [0.055, 0.042, 0.048], "bone": False},
    {"kind": "blob", "name": "cheek_ruff", "mirror": True, "attach": "mHead", "at": 0, "offset": [0.052, 0.05, 0.015], "radii": [0.036, 0.038, 0.042], "bone": "mFaceCheekUpperLeft", "pivot": "center", "blend": 0.03},
    {"kind": "blob", "name": "brow", "mirror": True, "attach": "mHead", "at": 0, "offset": [0.026, 0.097, 0.07], "radii": [0.016, 0.007, 0.013], "bone": "mFaceEyebrowCenterLeft", "pivot": "center", "blend": 0.01},
    {"kind": "chain", "name": "muzzle", "attach": "mHead", "at": 0, "offset": [0, 0.072, 0.055],
     "points": [[0, -0.006, 0.045], [0, -0.014, "$muzzle_length"]],
     "radius": [0.04, 0.03, 0.022], "blend": 0.03},
    {"kind": "blob", "name": "muzzle_top", "attach": "muzzle.0", "at": 0.6, "offset": [0, 0.008, 0], "radii": [0.03, 0.02, 0.045], "bone": False, "blend": 0.02},
    {"kind": "blob", "name": "nose_pad", "attach": "muzzle", "offset": [0, 0.006, 0.006], "radii": [0.017, 0.012, 0.013], "bone": "mFaceNoseCenter", "pivot": "center", "blend": 0.006},
    {"kind": "blob", "name": "nostril", "mirror": True, "op": "subtract", "global": True, "attach": "muzzle", "offset": [0.0075, 0.004, 0.018], "axis": [0.4, 0, 1], "radii": [0.0042, 0.003, 0.005], "blend": 0.0015},
    {"kind": "chain", "name": "jaw", "attach": "mHead", "at": 0, "offset": [0, 0.045, 0.0], "points": [[0, -0.032, "$muzzle_length+0.045"]], "bones": ["mFaceJaw"], "radius": [0.032, 0.016], "blend": 0.03, "limits": {"bend": [-5, 35]}},
    {"kind": "blob", "name": "chin", "attach": "mFaceJaw", "at": 0.8, "offset": [0, -0.006, 0], "radii": [0.016, 0.01, 0.018], "bone": "mFaceChin", "pivot": "center", "blend": 0.012},
    {"kind": "blob", "name": "flews", "mirror": True, "attach": "muzzle.0", "at": 0.55, "offset": [0.022, -0.02, 0], "radii": [0.012, 0.012, 0.035], "bone": "mFaceLipUpperLeft", "pivot": "center", "blend": 0.01},
    {"kind": "blob", "name": "lid_shell", "mirror": True, "attach": "mHead", "at": 0, "offset": [0.033, 0.083, 0.062], "axis": [0.35, 0.05, 1], "radii": [0.0125, 0.0115, 0.0122], "bone": "mFaceEyeLidUpperLeft", "pivot": "center", "blend": 0.004},
    {"kind": "blob", "name": "eye_opening", "mirror": True, "op": "subtract", "attach": "mHead", "at": 0, "offset": [0.0375, 0.0825, 0.073], "axis": [0.35, 0.05, 1], "radii": [0.0115, 0.0052, 0.0055], "blend": 0.0012},
    {"kind": "blob", "name": "eye", "mirror": True, "attach": "mHead", "at": 0, "offset": [0.033, 0.083, 0.0615], "radii": 0.011, "bone": "mEyeLeft", "pivot": "center", "blend": 0},
    {"kind": "scope", "name": "ear_kit", "mirror": True, "parts": [
        {"kind": "chain", "name": "ear", "attach": "mHead", "at": 0, "offset": [0.042, 0.15, -0.018],
         "points": [[0.012, "$ear_length*0.5", -0.004], [0.022, "$ear_length", -0.01]], "bones": ["mFaceEar1Left", "mFaceEar2Left"],
         "radius": [0.016, 0.01, 0.002], "blend": 0.018, "limits": {"bend": [-40, 40], "side": [-30, 30]}},
        {"kind": "blob", "name": "ear_flap", "attach": "mFaceEar1Left", "at": 0.55, "axis": [0.2, 0, 1], "radii": [0.03, 0.042, 0.008], "bone": False, "blend": 0.006},
        {"kind": "blob", "name": "ear_tip", "attach": "mFaceEar2Left", "at": 0.3, "axis": [0.2, 0, 1], "radii": [0.016, 0.03, 0.006], "bone": False, "blend": 0.006},
        {"kind": "blob", "name": "ear_cup", "op": "subtract", "attach": "mFaceEar1Left", "at": 0.7, "offset": [0, 0, 0.008], "axis": [0.2, 0, 1], "radii": [0.02, 0.045, 0.006], "blend": 0.003},
    ]},
]}

# ---------------------------------------------------------------- reptile head
reptile_head = {"kind": "scope", "name": "head", "scale": "$head_size", "parts": [
    {"kind": "blob", "name": "cranium", "attach": "mNeck", "offset": [0, 0.085, -0.01], "radii": [0.062, 0.066, 0.085], "bone": "mHead", "limits": {"bend": [-40, 40]}},
    {"kind": "chain", "name": "snout", "attach": "mHead", "at": 0, "offset": [0, 0.068, 0.045],
     "points": [[0, -0.008, 0.06], [0, -0.018, "$snout_length"]], "bones": ["mFaceNoseBridge", None],
     "radius": [0.038, 0.028, 0.017], "blend": 0.03},
    {"kind": "blob", "name": "snout_top", "attach": "mFaceNoseBridge", "at": 0.5, "offset": [0, 0.012, 0], "radii": [0.03, 0.016, 0.05], "bone": False, "blend": 0.02},
    {"kind": "blob", "name": "nostril", "mirror": True, "op": "subtract", "global": True, "attach": "snout", "offset": [0.009, 0.012, -0.004], "axis": [0.5, 0.4, 1], "radii": [0.0035, 0.0025, 0.0045], "blend": 0.0015},
    {"kind": "chain", "name": "brow_ridge", "mirror": True, "rigid": True, "attach": "mHead", "at": 0, "offset": [0.02, 0.102, 0.075],
     "points": [[0.014, 0.004, -0.025], [0.026, -0.002, -0.055]], "radius": [0.011, 0.012, 0.007], "blend": 0.012},
    {"kind": "chain", "name": "jaw", "attach": "mHead", "at": 0, "offset": [0, 0.038, -0.012], "points": [[0, -0.028, "$snout_length+0.06"]], "bones": ["mFaceJaw"], "radius": [0.034, 0.014], "blend": 0.03, "limits": {"bend": [-5, 40]}},
    {"kind": "blob", "name": "throat", "attach": "mFaceJaw", "at": 0.3, "offset": [0, -0.012, 0], "radii": [0.035, 0.02, 0.045], "bone": False, "blend": 0.025},
    {"kind": "blob", "name": "cheek", "mirror": True, "attach": "mHead", "at": 0, "offset": [0.045, 0.045, 0.02], "radii": [0.024, 0.026, 0.035], "bone": "mFaceCheekLowerLeft", "pivot": "center", "blend": 0.025},
    {"kind": "blob", "name": "lid_shell", "mirror": True, "attach": "mHead", "at": 0, "offset": [0.04, 0.084, 0.035], "axis": [1, 0.1, 0.5], "radii": [0.0125, 0.0118, 0.0122], "bone": "mFaceEyeLidUpperLeft", "pivot": "center", "blend": 0.004},
    {"kind": "blob", "name": "eye_opening", "mirror": True, "op": "subtract", "attach": "mHead", "at": 0, "offset": [0.0505, 0.0838, 0.0405], "axis": [1, 0.1, 0.5], "radii": [0.0115, 0.0058, 0.0055], "blend": 0.0012},
    {"kind": "blob", "name": "eye", "mirror": True, "attach": "mHead", "at": 0, "offset": [0.04, 0.084, 0.035], "radii": 0.011, "bone": "mEyeLeft", "pivot": "center", "blend": 0},
    {"kind": "blob", "name": "ear_hole", "mirror": True, "op": "subtract", "attach": "mHead", "at": 0, "offset": [0.06, 0.07, -0.03], "axis": [1, 0, 0.3], "radii": [0.009, 0.008, 0.004], "blend": 0.003},
    {"kind": "chain", "name": "horn", "mirror": True, "attach": "mHead", "at": 0, "offset": [0.032, 0.125, -0.035],
     "dir": [0.3, 0.45, -1], "length": "$horn_length", "segments": 4, "curl": "-$horn_curl",
     "radius": [0.016, 0.003], "blend": 0.012, "limits": {"bend": [0, 0], "side": [0, 0], "twist": [0, 0]}},
    {"kind": "array", "name": "crest", "along": "horn", "count": 3, "range": [0.15, 0.7], "side": [0, 1, 0], "size": [0.006, 0.012], "taper": 0.6, "lean": -15, "mirror": True},
]}

def program(name, params, parts):
    return {"name": name, "skeleton": "second_life", "params": params,
            "settings": {"blend": 0.035, "resolution": 900, "triangles": 60000}, "parts": parts}

torso = take("spine", "hips", "glute", "abdomen", "ribcage", "trapezius", "neck", "neck_muscle")
arms = take("arm", "deltoid", "biceps", "forearm", "palm")

canine = program("canine", {
    **{k: v for k, v in human["params"].items() if k in ("hip_height", "torso_length", "shoulder_width", "arm_length", "hand_size", "head_size", "build")},
    "muzzle_length": {"value": 0.085, "min": 0.04, "max": 0.14},
    "ear_length": {"value": 0.1, "min": 0.05, "max": 0.17},
    "tail_length": {"value": 0.75, "min": 0.3, "max": 1.2},
    "tail_curl": {"value": 25, "min": -40, "max": 90},
}, torso + [
    {"kind": "blob", "name": "chest_fluff", "attach": "mChest", "at": 0.6, "offset": [0, -0.01, 0.075], "radii": [0.095, 0.12, 0.05], "bone": False},
    canine_head,
] + arms + hand(["index", "middle", "ring", "pinky", "thumb"], 0.8, 1.25, -18)
  + claws(["index", "middle", "ring", "pinky", "thumb"], 0.016, 0.0045)
  + digitigrade(1.0) + [
    {"kind": "chain", "name": "tail", "attach": "mPelvis", "at": 0, "offset": [0, -0.02, -0.11], "dir": [0, -0.35, -1],
     "length": "$tail_length", "segments": 6, "curl": "$tail_curl", "bones": ["mTail1", "mTail2", "mTail3", "mTail4", "mTail5", "mTail6"],
     "radius": [0.045, 0.075, 0.085, 0.07, 0.045, 0.012], "limits": {"bend": [-40, 40], "side": [-45, 45]}},
])

scaly = program("scaly", {
    **{k: v for k, v in human["params"].items() if k in ("hip_height", "torso_length", "shoulder_width", "arm_length", "hand_size", "head_size", "build")},
    "snout_length": {"value": 0.1, "min": 0.05, "max": 0.17},
    "horn_length": {"value": 0.12, "min": 0.0, "max": 0.25},
    "horn_curl": {"value": 60, "min": 0, "max": 150},
    "tail_length": {"value": 1.1, "min": 0.5, "max": 1.8},
    "tail_curl": {"value": -15, "min": -60, "max": 60},
    "spike_size": {"value": 0.035, "min": 0.0, "max": 0.08},
}, torso + [
    reptile_head,
    {"kind": "array", "name": "back_spikes", "along": "spine", "count": 6, "range": [0.05, 0.95], "side": [0, 0.3, -1], "size": [0.016, "$spike_size"], "lean": 20},
    {"kind": "array", "name": "neck_spikes", "along": "neck", "count": 2, "range": [0.2, 0.8], "side": [0, 0.3, -1], "size": [0.012, "$spike_size*0.7"], "lean": 20},
    {"kind": "array", "name": "belly_plates", "along": "spine", "count": 5, "range": [0.12, 0.85], "side": [0, 0, 1], "shape": "plate", "size": [0.05, 0.022], "blend": 0.01},
] + arms + hand(["index", "middle", "ring", "thumb"], 0.95, 1.3, -20)
  + claws(["index", "middle", "ring", "thumb"], 0.022, 0.005)
  + digitigrade(1.12) + [
    {"kind": "chain", "name": "tail", "attach": "mPelvis", "at": 0, "offset": [0, -0.02, -0.1], "dir": [0, -0.3, -1],
     "length": "$tail_length", "segments": 6, "curl": "$tail_curl", "bones": ["mTail1", "mTail2", "mTail3", "mTail4", "mTail5", "mTail6"],
     "radius": [0.11, 0.085, 0.06, 0.04, 0.022, 0.008], "limits": {"bend": [-30, 30], "side": [-40, 40]}},
    {"kind": "array", "name": "tail_spikes", "along": "tail", "count": 10, "range": [0.02, 0.85], "side": [0, 1, 0], "size": [0.014, "$spike_size"], "taper": 0.3, "lean": -25},
])

def dump(v, ind=""):
    """JSON with short arrays and small objects on one line (matches the Studio formatter)."""
    def flat(x):
        return not isinstance(x, (dict, list))

    if isinstance(v, list):
        if all(flat(x) for x in v):
            return "[" + ", ".join(json.dumps(x) for x in v) + "]"
        return "[\n" + ",\n".join(ind + "  " + dump(x, ind + "  ") for x in v) + "\n" + ind + "]"
    if isinstance(v, dict):
        inner = [json.dumps(k) + ": " + dump(x, ind + "  ") for k, x in v.items()]
        one = "{" + ", ".join(inner) + "}"
        simple = all(flat(x) or (isinstance(x, list) and all(flat(y) or (isinstance(y, list) and all(flat(z) for z in y)) for y in x)) for x in v.values())
        if simple and len(one) + len(ind) <= 150:
            return one
        return "{\n" + ",\n".join(ind + "  " + s for s in inner) + "\n" + ind + "}"
    return json.dumps(v)

for prog in (canine, scaly):
    open(PROGRAMS + prog["name"] + ".json", "w").write(dump(prog) + "\n")
    print("wrote", prog["name"], len(prog["parts"]), "parts")
