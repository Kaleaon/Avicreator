"""Fits muscle parts for a shape program to sampled anatomy.

    python -I fit_muscles.py muscles.npz skeleton.json [program.json] [--skin skin.obj]

`muscles.npz` comes from zanatomy_muscles.py; `skeleton.json` from
`avishape skeleton program.json --pose anatomical`, so the bones stand where
the anatomy does. Every entry in MUSCLES becomes one or more `muscle` parts
(left side, mirrored): a spindle along the sample cloud's principal axis,
with the belly's cross-section from its second moments, and each end
anchored to the nearest of the listed bones in that bone's rest frame.
Each belly bulges toward the body surface: toward the nearest vertex of
the reference skin when one is given, else away from the nearest bone.

With a program path, its `musculature` scope is replaced in place (the
rest of the file is left untouched); otherwise the scope is printed.
Sizes are written as `r*$muscle+$fat`, so programs need `muscle` and `fat`
params.

The fitted numbers derive from Z-Anatomy (CC-BY-SA 4.0), itself derived
from BodyParts3D (CC-BY-SA 2.1 Japan); credit both wherever they are used.
"""
import json
import re
import sys

import numpy as np

SPINE = ["mPelvis", "mTorso", "mChest"]

# name, Z-Anatomy meshes (left side) and candidate bones for each end. A
# muscle may split into heads: "meshes" makes one head per mesh (biceps),
# {"fan": k} slices a fan-shaped sheet by angle around its insertion,
# {"kmeans": k} cuts a broad sheet into patches, {"along": [...]} cuts it
# into segments at fractions of its length from the origin end. Heads merge
# smoothly (radius `smooth`) unless `separate`, which makes each its own
# part, divided from the next by a groove (the rectus "pack"). `lateral`
# keeps only samples at least that far from the midline; `raise_by` scales
# each head's relief height. Deep muscles (rhomboids, vastus intermedius)
# are left out: as relief they would show through what covers them.
MUSCLES = [
    # Chest and abdomen.
    dict(name="pec_clavicular", meshes=["Clavicular head of pectoralis major muscle.l"],
         origin=["mCollarLeft", "mChest"], insertion=["mShoulderLeft"]),
    dict(name="pec_sternal", meshes=["Sternocostal head of pectoralis major muscle.l"],
         origin=["mChest"], insertion=["mShoulderLeft"], split={"fan": 2}, smooth=0.035),
    dict(name="pec_abdominal", meshes=["(Abdominal part of pectoralis major muscle).l"],
         origin=["mChest", "mTorso"], insertion=["mShoulderLeft"]),
    # Tendinous intersections split the rectus into the "pack": one long
    # segment below the navel and three above.
    dict(name="rectus_abdominis", meshes=["Rectus abdominis muscle.l"],
         origin=["mPelvis", "mTorso"], insertion=SPINE, split={"along": [0, 0.42, 0.62, 0.81, 1]}, separate=True,
         raise_by=[0.5, 1, 1, 1]),
    dict(name="external_oblique", meshes=["External abdominal oblique muscle.l"],
         origin=["mChest", "mTorso"], insertion=["mPelvis", "mTorso"], split={"kmeans": 2}, smooth=0.035,
         lateral=0.075),
    dict(name="serratus_anterior", meshes=["Serratus anterior muscle.l"],
         origin=["mChest"], insertion=["mChest"], split={"kmeans": 6}, smooth=0.006),
    # Back.
    dict(name="latissimus", meshes=["Latissimus dorsi muscle.l"],
         origin=SPINE, insertion=["mShoulderLeft"], split={"fan": 3}, smooth=0.035),
    dict(name="trapezius_upper", meshes=["Ascending part of trapezius muscle.l"],
         origin=["mHead", "mNeck"], insertion=["mCollarLeft"]),
    dict(name="trapezius_middle", meshes=["Transverse part of trapezius muscle.l"],
         origin=["mChest", "mNeck"], insertion=["mCollarLeft"]),
    dict(name="trapezius_lower", meshes=["Descending part of trapezius muscle.l"],
         origin=["mChest", "mTorso"], insertion=["mCollarLeft"]),
    dict(name="infraspinatus", meshes=["Infraspinatus muscle.l"],
         origin=["mCollarLeft"], insertion=["mShoulderLeft"]),
    dict(name="teres_major", meshes=["Teres major muscle.l"],
         origin=["mCollarLeft"], insertion=["mShoulderLeft"]),
    dict(name="erector_longissimus", meshes=["Longissimus thoracis muscle.l"],
         origin=["mPelvis", "mTorso"], insertion=SPINE, split={"along": [0, 0.5, 1]}, smooth=0.015),
    dict(name="erector_iliocostalis", meshes=["Iliocostalis lumborum muscle.l"],
         origin=["mPelvis"], insertion=["mTorso", "mChest"]),
    # Neck.
    dict(name="sternocleidomastoid", meshes=["Sternocleidomastoid muscle.l"],
         origin=["mHead"], insertion=["mChest", "mCollarLeft"]),
    dict(name="levator_scapulae", meshes=["Levator scapulae.l"],
         origin=["mNeck", "mHead"], insertion=["mCollarLeft"]),
    # Shoulder and upper arm.
    dict(name="deltoid_front", meshes=["Clavicular part of deltoid muscle.l"],
         origin=["mCollarLeft"], insertion=["mShoulderLeft"]),
    dict(name="deltoid_side", meshes=["Acromial part of deltoid muscle.l"],
         origin=["mCollarLeft"], insertion=["mShoulderLeft"]),
    dict(name="deltoid_rear", meshes=["Scapular spinal part of deltoid muscle.l"],
         origin=["mCollarLeft"], insertion=["mShoulderLeft"]),
    dict(name="biceps", meshes=["Long head of biceps brachii.l", "Short head of biceps brachii.l"],
         origin=["mShoulderLeft", "mCollarLeft"], insertion=["mElbowLeft"], split="meshes"),
    dict(name="brachialis", meshes=["Brachialis muscle.l"],
         origin=["mShoulderLeft"], insertion=["mElbowLeft"]),
    dict(name="triceps", meshes=["Long head of triceps brachii.l", "Lateral head of triceps brachii.l",
                                 "Medial head of triceps brachii.l"],
         origin=["mCollarLeft", "mShoulderLeft"], insertion=["mElbowLeft", "mShoulderLeft"], split="meshes"),
    # Forearm: most bellies start on the humeral epicondyles.
    dict(name="brachioradialis", meshes=["Brachioradialis muscle.l"],
         origin=["mShoulderLeft"], insertion=["mElbowLeft"]),
    dict(name="ext_carpi_radialis_longus", meshes=["Extensor carpi radialis longus.l"],
         origin=["mShoulderLeft"], insertion=["mElbowLeft"]),
    dict(name="ext_carpi_radialis_brevis", meshes=["Extensor carpi radialis brevis.l"],
         origin=["mShoulderLeft", "mElbowLeft"], insertion=["mElbowLeft"]),
    dict(name="extensor_digitorum", meshes=["Extensor digitorum.l"],
         origin=["mShoulderLeft", "mElbowLeft"], insertion=["mElbowLeft", "mWristLeft"]),
    dict(name="ext_carpi_ulnaris", meshes=["Humeral head of extensor carpi ulnaris.l"],
         origin=["mShoulderLeft", "mElbowLeft"], insertion=["mElbowLeft"]),
    dict(name="anconeus", meshes=["Anconeus muscle.l"],
         origin=["mShoulderLeft"], insertion=["mElbowLeft"]),
    dict(name="flex_carpi_ulnaris", meshes=["Humeral head of flexor carpi ulnaris.l", "Ulnar head of flexor carpi ulnaris.l"],
         origin=["mShoulderLeft", "mElbowLeft"], insertion=["mElbowLeft"]),
    dict(name="flex_carpi_radialis", meshes=["Flexor carpi radialis.l"],
         origin=["mShoulderLeft", "mElbowLeft"], insertion=["mElbowLeft"]),
    dict(name="palmaris_longus", meshes=["Palmaris longus muscle.l"],
         origin=["mShoulderLeft", "mElbowLeft"], insertion=["mElbowLeft", "mWristLeft"]),
    dict(name="flex_digitorum_superficialis", meshes=["Humero-ulnar head of flexor digitorum superficialis.l"],
         origin=["mShoulderLeft", "mElbowLeft"], insertion=["mElbowLeft", "mWristLeft"]),
    dict(name="pronator_teres", meshes=["Superficial head of pronator teres.l", "Deep head of pronator teres.l"],
         origin=["mShoulderLeft"], insertion=["mElbowLeft"]),
    # Hand pads.
    dict(name="thenar", meshes=["Abductor pollicis brevis.l", "Superficial head of flexor pollicis brevis.l",
                                "Opponens pollicis muscle.l"],
         origin=["mWristLeft"], insertion=["mWristLeft"]),
    dict(name="hypothenar", meshes=["Abductor digiti minimi of hand.l", "Flexor digiti minimi of hand.l",
                                    "Opponens digiti minimi muscle of hand.l"],
         origin=["mWristLeft"], insertion=["mWristLeft"]),
    # Hip and thigh.
    dict(name="gluteus_maximus", meshes=["Gluteus maximus muscle.l"],
         origin=["mPelvis"], insertion=["mHipLeft", "mPelvis"], split={"kmeans": 2}, smooth=0.04),
    dict(name="gluteus_medius", meshes=["Gluteus medius muscle.l"],
         origin=["mPelvis"], insertion=["mHipLeft"]),
    dict(name="tensor_fasciae_latae", meshes=["Tensor fasciae latae.l"],
         origin=["mPelvis"], insertion=["mHipLeft"]),
    dict(name="rectus_femoris", meshes=["Rectus femoris muscle.l"],
         origin=["mPelvis", "mHipLeft"], insertion=["mHipLeft"]),
    dict(name="vastus_lateralis", meshes=["Vastus lateralis muscle.l"],
         origin=["mHipLeft"], insertion=["mHipLeft"]),
    dict(name="vastus_medialis", meshes=["Vastus medialis muscle.l"],
         origin=["mHipLeft"], insertion=["mHipLeft"]),
    dict(name="sartorius", meshes=["Sartorius muscle.l"],
         origin=["mPelvis", "mHipLeft"], insertion=["mHipLeft", "mKneeLeft"], split={"along": [0, 0.5, 1]}, smooth=0.01),
    dict(name="gracilis", meshes=["Gracilis muscle.l"],
         origin=["mPelvis", "mHipLeft"], insertion=["mKneeLeft", "mHipLeft"]),
    dict(name="adductor_longus", meshes=["Adductor longus.l"],
         origin=["mPelvis", "mHipLeft"], insertion=["mHipLeft"]),
    dict(name="adductor_magnus", meshes=["Adductor magnus.l"],
         origin=["mPelvis", "mHipLeft"], insertion=["mHipLeft"]),
    dict(name="biceps_femoris", meshes=["Long head of biceps femoris.l"],
         origin=["mPelvis", "mHipLeft"], insertion=["mKneeLeft", "mHipLeft"]),
    dict(name="semitendinosus", meshes=["Semitendinosus muscle.l"],
         origin=["mPelvis", "mHipLeft"], insertion=["mKneeLeft", "mHipLeft"]),
    dict(name="semimembranosus", meshes=["Semimembranosus muscle.l"],
         origin=["mPelvis", "mHipLeft"], insertion=["mKneeLeft", "mHipLeft"]),
    # Lower leg.
    dict(name="gastrocnemius_medial", meshes=["Medial head of gastrocnemius.l"],
         origin=["mHipLeft", "mKneeLeft"], insertion=["mKneeLeft"]),
    dict(name="gastrocnemius_lateral", meshes=["Lateral head of gastrocnemius.l"],
         origin=["mHipLeft", "mKneeLeft"], insertion=["mKneeLeft"]),
    dict(name="soleus", meshes=["Soleus muscle.l"],
         origin=["mKneeLeft"], insertion=["mKneeLeft", "mAnkleLeft"]),
    dict(name="calcaneal_tendon", meshes=["Calcaneal tendon.l"],
         origin=["mKneeLeft"], insertion=["mAnkleLeft", "mKneeLeft"]),
    dict(name="tibialis_anterior", meshes=["Tibialis anterior muscle.l"],
         origin=["mKneeLeft"], insertion=["mKneeLeft", "mAnkleLeft"]),
    dict(name="fibularis_longus", meshes=["Fibularis longus muscle.l"],
         origin=["mKneeLeft"], insertion=["mKneeLeft", "mAnkleLeft"]),
    dict(name="extensor_digitorum_longus", meshes=["Extensor digitorum longus.l"],
         origin=["mKneeLeft"], insertion=["mKneeLeft", "mAnkleLeft"]),
]


def quat_rotate(q, v):
    """Rotates v by the unit quaternion q = [x, y, z, w]."""
    u, w = np.asarray(q[:3]), q[3]
    return 2 * np.dot(u, v) * u + (w * w - np.dot(u, u)) * v + 2 * w * np.cross(u, v)


def conj(q):
    return [-q[0], -q[1], -q[2], q[3]]


def kmeans(points, k, iterations=40):
    """Deterministic k-means seeded along the principal axis."""
    centred = points - points.mean(0)
    axis = np.linalg.eigh(np.cov(centred.T))[1][:, -1]
    order = np.argsort(centred @ axis)
    centres = np.array([points[order[int((i + 0.5) * len(points) / k)]] for i in range(k)])
    for _ in range(iterations):
        labels = np.argmin(((points[:, None, :] - centres[None]) ** 2).sum(-1), axis=1)
        new = np.array([points[labels == i].mean(0) if np.any(labels == i) else centres[i] for i in range(k)])
        if np.allclose(new, centres, atol=1e-6):
            break
        centres = new
    return [points[labels == i] for i in range(k)]


def fan(points, k, entry, bones):
    """Slices a fan-shaped sheet (pectoral, latissimus) into `k` heads by
    angle around its apex, the end nearest the insertion bones."""
    centre = points.mean(0)
    values, vectors = np.linalg.eigh(np.cov((points - centre).T))
    e1, e2 = vectors[:, -1], vectors[:, -2]
    s = (points - centre) @ e1
    lo, hi = np.percentile(s, [2, 98])
    ends = [points[s <= lo].mean(0), points[s >= hi].mean(0)]
    apex = min(ends, key=lambda e: nearest_bone(e, entry["insertion"], bones)[1])
    rel = points - apex
    angle = np.arctan2(rel @ e2, rel @ e1)
    # Unwrap around the far side of the apex so slices stay contiguous.
    far = np.arctan2(np.mean(rel @ e2), np.mean(rel @ e1))
    angle = (angle - far + np.pi) % (2 * np.pi)
    cuts = np.quantile(angle, np.linspace(0, 1, k + 1))
    cuts[-1] += 1e-9
    return [points[(angle >= a) & (angle < b)] for a, b in zip(cuts, cuts[1:])]


def principal_axis(points):
    centre = points.mean(0)
    values, vectors = np.linalg.eigh(np.cov((points - centre).T))
    return centre, vectors[:, -1]


def fit_spindle(points):
    """Origin/insertion points, belly span and cross-section of a cloud."""
    centre, axis = principal_axis(points)
    s = (points - centre) @ axis
    lo, hi = np.percentile(s, [0.5, 99.5])
    length = hi - lo
    # Cross-sectional area along the axis: the belly is where it stays
    # above 80% of its peak, its size the second moments there.
    bins = 20
    counts, edges = np.histogram(s, bins=bins, range=(lo, hi))
    smooth = np.convolve(counts, [1, 2, 1], mode="same") / 4.0
    peak = int(np.argmax(smooth))
    full = smooth >= 0.8 * smooth[peak]
    start, end = peak, peak
    while start > 0 and full[start - 1]:
        start -= 1
    while end < bins - 1 and full[end + 1]:
        end += 1
    b0 = (edges[start] + edges[start + 1]) / 2
    b1 = (edges[end] + edges[end + 1]) / 2
    section = points[(s >= edges[start]) & (s <= edges[end + 1])]
    flat = section - np.outer((section - centre) @ axis, axis)
    flat_centre = flat.mean(0)
    values, vectors = np.linalg.eigh(np.cov((flat - flat_centre).T))
    # The two largest in-plane moments (the third is ~0 along the axis).
    order = np.argsort(values)[::-1][:2]
    # Every station holds a whole cross-section, so the stretch's moments
    # are a cross-section's: a filled ellipse has variance r^2 / 4.
    width, thickness = (2 * np.sqrt(max(values[i], 1e-10)) for i in order)
    normal = vectors[:, order[1]]
    # Keep the belly on the line: shift it through the section's centre.
    shift = flat_centre - centre
    shift -= axis * np.dot(shift, axis)
    a = centre + shift + axis * lo
    b = centre + shift + axis * hi
    belly = [float((b0 - lo) / length), float((b1 - lo) / length)]
    covariance = np.cov((flat - flat_centre).T)
    return dict(ends=(a, b), belly=belly, size=[float(width), float(thickness)], normal=normal, length=float(length),
                axis=axis, centre=flat_centre + axis * (b0 + b1) / 2,
                covariance=covariance)


def outward(fit, skin, bones):
    """Unit direction from the belly toward the body surface, across the
    muscle: toward the nearest skin point, or away from the nearest bone."""
    centre, axis = fit["centre"], fit["axis"]
    if skin is not None:
        direction = skin[np.argmin(((skin - centre) ** 2).sum(1))] - centre
    else:
        name = nearest_bone(centre, list(bones), bones)[0]
        direction = centre - closest_on_segment(centre, bones[name]["head"], bones[name]["tail"])[0]
    direction = direction - axis * np.dot(direction, axis)
    norm = np.linalg.norm(direction)
    return direction / norm if norm > 1e-9 else fit["normal"]


def oriented_size(fit, normal):
    """Half-width and half-thickness of the belly with thickness along `normal`."""
    across = np.cross(normal, fit["axis"])
    c = fit["covariance"]
    return [float(2 * np.sqrt(max(across @ c @ across, 1e-10))), float(2 * np.sqrt(max(normal @ c @ normal, 1e-10)))]


def split_cloud(clouds, entry, bones):
    split = entry.get("split")
    if split == "meshes":
        return clouds
    points = np.concatenate(clouds)
    if not split:
        return [points]
    if "kmeans" in split:
        return kmeans(points, split["kmeans"])
    if "fan" in split:
        return fan(points, split["fan"], entry, bones)
    # Cut along the principal axis, measured from the end nearest the origin bones.
    centre, axis = principal_axis(points)
    s = (points - centre) @ axis
    lo, hi = np.percentile(s, [0.5, 99.5])
    end_lo, end_hi = centre + axis * lo, centre + axis * hi
    if nearest_bone(end_hi, entry["origin"], bones)[1] < nearest_bone(end_lo, entry["origin"], bones)[1]:
        s, lo, hi = -s, -hi, -lo
    cuts = [lo + f * (hi - lo) for f in split["along"]]
    cuts[0], cuts[-1] = s.min() - 1, s.max() + 1
    return [points[(s >= a) & (s < b)] for a, b in zip(cuts, cuts[1:])]


def closest_on_segment(p, a, b):
    ab = b - a
    t = float(np.clip(np.dot(p - a, ab) / max(np.dot(ab, ab), 1e-12), 0.0, 1.0))
    return a + ab * t, t


def nearest_bone(p, names, bones):
    best = None
    for name in names:
        bone = bones[name]
        c, t = closest_on_segment(p, bone["head"], bone["tail"])
        d = float(np.linalg.norm(p - c))
        if best is None or d < best[1]:
            best = (name, d, t)
    return best


def anchor(p, name, t, bones):
    """{attach, at, offset}: the point on the bone at `t`, plus an offset in
    the bone's rest frame (the compiler turns it by the bone's pose)."""
    bone = bones[name]
    base = bone["head"] + (bone["tail"] - bone["head"]) * t
    offset = quat_rotate(conj(bone["pose"]), p - base)
    return {"attach": name, "at": round(t, 3), "offset": [round(float(x), 4) for x in offset]}


def size_expr(r):
    return f"{r:.4f}*$muscle"


def fit_head(piece, entry, bones, skin):
    fit = fit_spindle(piece)
    # Bulge toward the surface; thickness is measured that way.
    fit["normal"] = outward(fit, skin, bones)
    fit["size"] = oriented_size(fit, fit["normal"])
    a, b = fit["ends"]
    # Each end rides on its nearest candidate bone (a patch of a sheet that
    # stops short of the insertion stays on the trunk); the origin is the
    # end on an origin bone.
    candidates = entry["origin"] + [n for n in entry["insertion"] if n not in entry["origin"]]
    ao, ai = nearest_bone(a, candidates, bones), nearest_bone(b, candidates, bones)
    belly = fit["belly"]
    swap = ao[0] not in entry["origin"] and ai[0] in entry["origin"]
    if ao[0] in entry["origin"] and ai[0] in entry["origin"]:
        # Both on origin bones: the origin is the end farther from the insertion bones.
        swap = nearest_bone(a, entry["insertion"], bones)[1] < nearest_bone(b, entry["insertion"], bones)[1]
    if swap:
        a, b, ao, ai = b, a, ai, ao
        belly = [1 - belly[1], 1 - belly[0]]
    # Keep a taper of at least 30% of the length at each end: a flat belly
    # that stops short would end in a step.
    b0, b1 = min(max(belly[0], 0.3), 0.7), min(max(belly[1], 0.3), 0.7)
    belly = [round(b0, 2), round(max(b0, b1), 2)]
    normal = quat_rotate(conj(bones[ao[0]]["pose"]), fit["normal"])
    head = {
        "origin": anchor(a, ao[0], ao[2], bones),
        "insertion": anchor(b, ai[0], ai[2], bones),
        "size": [size_expr(fit["size"][0]), size_expr(fit["size"][1])],
        "normal": [round(float(x), 3) for x in normal],
        "belly": belly[0] if belly[0] == belly[1] else belly,
    }
    summary = f"{fit['length'] * 1000:4.0f} mm, {fit['size'][0] * 1000:4.1f} x {fit['size'][1] * 1000:4.1f}, {ao[0]} -> {ai[0]}"
    return head, summary


def fit_all(samples, bones, skin=None):
    parts = []
    for entry in MUSCLES:
        clouds = [samples[m] for m in entry["meshes"] if m in samples]
        if "lateral" in entry:
            # Only the part beside the rectus sheath (|x| beyond the limit).
            clouds = [c[np.abs(c[:, 0]) >= entry["lateral"]] for c in clouds]
        if len(clouds) < len(entry["meshes"]):
            missing = [m for m in entry["meshes"] if m not in samples]
            print(f"{entry['name']}: no samples for {missing}", file=sys.stderr)
        if not clouds:
            continue
        pieces = [p for p in split_cloud(clouds, entry, bones) if len(p) >= 30]
        fitted = [fit_head(p, entry, bones, skin) for p in pieces]
        for (head, _), k in zip(fitted, entry.get("raise_by", [])):
            # Relief height follows the half-thickness; flatten covered or
            # thin stretches (the rectus below the navel lies under fat).
            head["size"][1] = head["size"][1].replace("*$muscle", f"*{k}*$muscle") if k != 1 else head["size"][1]
        groups = [[f] for f in fitted] if entry.get("separate") else [fitted]
        for i, group in enumerate(groups):
            name = entry["name"] if len(groups) == 1 else f"{entry['name']}_{i + 1}"
            part = {"kind": "muscle", "name": name, "mirror": True}
            if len(group) == 1:
                part.update(group[0][0])
            else:
                part["heads"] = [head for head, _ in group]
                if "smooth" in entry:
                    part["smooth"] = entry["smooth"]
            part["blend"] = "0.002+0.5*$fat"
            parts.append(part)
            for _, summary in group:
                print(f"{name:30} {summary}", file=sys.stderr)
    return parts


def scope_text(parts, indent="    "):
    lines = [f'{indent}{{\n{indent}  "kind": "scope", "name": "musculature",\n{indent}  "parts": [']
    body = []
    for p in parts:
        if "heads" in p:
            heads = p.pop("heads")
            head_lines = ",\n".join(f"{indent}        {json.dumps(h)}" for h in heads)
            text = json.dumps(p)[:-1] + ', "heads": [\n' + head_lines + f"\n{indent}      ]}}"
            body.append(f"{indent}    " + text)
        else:
            body.append(f"{indent}    {json.dumps(p)}")
    lines.append(",\n".join(body))
    lines.append(f"{indent}  ]\n{indent}}}")
    return "\n".join(lines)


def load_obj_vertices(path):
    with open(path) as f:
        return np.array([[float(x) for x in line.split()[1:4]] for line in f if line.startswith("v ")])


def main():
    args = sys.argv[1:]
    skin = None
    if "--skin" in args:
        i = args.index("--skin")
        skin = load_obj_vertices(args[i + 1])
        del args[i:i + 2]
    samples_path, skeleton_path, *program = args
    data = np.load(samples_path)
    samples = {k: data[k].astype(np.float64) for k in data.files if "#" not in k}
    skeleton = json.load(open(skeleton_path))
    bones = {b["name"]: {"head": np.array(b["head"]), "tail": np.array(b["tail"]), "pose": b["pose"]}
             for b in skeleton["bones"]}
    parts = fit_all(samples, bones, skin)
    text = scope_text(parts)
    if not program:
        print(text)
        return
    path = program[0]
    source = open(path).read()
    pattern = re.compile(r'    \{\n      "kind": "scope", "name": "musculature",\n.*?\n    \}', re.S)
    if pattern.search(source):
        source = pattern.sub(lambda _: text, source, count=1)
    else:
        # Append as the last part.
        source = re.sub(r"\n  \]\n\}\s*$", ",\n" + text + "\n  ]\n}\n", source)
    open(path, "w").write(source)
    print(f"wrote {len(parts)} muscles into {path}", file=sys.stderr)


if __name__ == "__main__":
    main()
