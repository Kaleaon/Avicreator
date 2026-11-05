# Muscle modelling and shape shifting

The nine built-in forms are human, canine, feline, vulpine, minotaur, centaur,
scaly, lizard and dragon. Shape Studio can load them, edit muscle mass and
separation, inspect a flex pose, and shift between forms. The custom centaur rig
has a human upper body, four equine legs and an articulated tail; it exports to
GLB. Compatible biped forms retain their Second Life skeleton validation.

## Authoring muscles

A muscle creates geometry between two existing rig attachments, without adding
bones. Its fusiform belly narrows into tendons at both endpoints. For example:

```json
{
  "kind": "muscle",
  "name": "biceps",
  "mirror": true,
  "origin": {"attach": "mShoulderLeft", "at": 0.15, "offset": [0, 0, 0.028]},
  "insertion": {"attach": "mElbowLeft", "at": 0.12, "offset": [0, 0, 0.018]},
  "radius": "0.042*$build*$muscle_mass",
  "tendon": 0.2,
  "preserve_volume": true,
  "blend": "0.024*(1-0.75*$muscle_definition)"
}
```

Both bones must already exist. A part name resolves to its last bone; use an
explicit segment or joint name to attach elsewhere. `at` defaults to 0.5 and
must be within 0..1. Offsets use authored world axes, matching existing parts;
they rotate with the attached bone in a named pose. Scopes scale offsets and
radii, and mirroring resolves the corresponding right-side bones.

`radius` must be positive. `tendon` is the endpoint radius divided by maximum
belly radius, within (0,1], defaulting to 0.2. Coincident attachments are rejected.
The profile uses twelve tapered round-cone sections, evaluated identically for
CPU meshing and GLSL field previews. Skin shares cross-fade smoothly along the
line between the two attachments. Identical attachment bones stay rigid.

In a named pose, radial scale is approximately `sqrt(rest_length/posed_length)`.
It is bounded to 0.5..2 to avoid unbounded bulges in extreme poses. This preserves
approximate swept-profile volume before caps and surface blends; it is an
artistic deformation model, not a biomechanical simulation. `preserve_volume:
false` keeps the authored radius. Exported generic flex animation uses normal
skeletal skinning; muscle bulges are baked by compiling named poses.

Shared recipes include antagonist pairs, pectoral fan bundles, abdominal
segments, obliques, latissimus, tapered distal limbs, and proximal tail/wing
muscles. `muscle_mass` adjusts belly size. `muscle_definition` reduces blending
between masses. Form-specific proportions stay independent of those controls.

Regenerate additional forms with:

```sh
python3 crates/avicreator-shape/programs/make_anthro.py
python3 crates/avicreator-shape/programs/make_forms.py
```

The shared recipes are in `programs/muscles.py`; regeneration retains authored
muscle control values and replaces existing recipes rather than duplicating them.

## Shape shifting

```sh
cargo run --release -p avicreator-shape --bin avishape -- studio crates/avicreator-shape/programs/human.json
cargo run --release -p avicreator-shape --bin avishape -- build crates/avicreator-shape/programs/human.json --target centaur --progress 0.5 --resolution 96 --out work/shift
```

In Studio, choose a target and move Transformation from 0% to 100%. Export
captures the current transformation and pose. Loading a form replaces the editor
contents; Save writes those contents to the open source file. Transformation
controls are preview/export state, so Save keeps an editable source program.

Rust callers use `shapeshift::blend_forms` or `shapeshift::preview_shift`. WASM
callers use `shape_forms`, `shape_shift_compile` and `shape_shift_build_glb`.
CLI targets can be built-in names or JSON paths. Named poses used during a shift
must exist in both forms; all built-in forms provide `muscle_flex`.

Matching named parts with compatible primitive layouts interpolate positions,
radii and orientations. Unmatched anatomy grows or shrinks around shared rig
attachments, using the closest shared bone when no shared ancestor exists. This
keeps a recognizable torso through biped-to-centaur transitions and supports
different part/limb counts without vertex correspondence. Shared names establish
anatomical correspondence for custom forms; completely different naming
conventions use a centre-based collapse fallback.
Endpoints at exactly 0 and 1 retain the original compiled form and skeleton.
Intermediate snapshots use two namespaced rigs and normalized vertex weights;
the same compiled primitives drive field previews, mesh previews and exports.
The forms should be aligned in the same authored coordinate system. An artist should inspect intermediate frames and author explicit correspondence
or bridge forms where differently named anatomy should follow a specific path.

An intermediate GLB is a remeshed skinned snapshot, not a fixed-topology glTF
morph animation. The transformation is not exported as a timeline clip, and
intermediate rigs do not target Second Life. Studio defaults to a raster mesh
preview for responsive camera interaction; the implicit field remains available
for inspecting the continuous surface. Standalone build previews use the
exported mesh. Fine features should be reviewed at an appropriate export
resolution. Studio offers Draft, Detailed and Fine mesh previews (64, 96 and
160 cells respectively), so finer anatomy can be reviewed before export.

## Photo reference review

The following photographs were downloaded to the task's scratch directory and
visually inspected. They are linked here for attribution and repeatable review;
photo files are not distributed in this repository.

| Reference | Observations used in the recipes |
| --- | --- |
| [Standing horse profile, Kerry M. Thomas](https://www.kerrymthomas.com/post/the-horse-before-the-system-reframing-equine-welfare-through-the-horse-s-lived-experience) | Broad barrel and croup, distinct shoulder mass, stifle forward of the hock, and slim distal legs. The centaur uses these equine landmarks below its fantasy torso junction. |
| [H031 bull photograph, Kniebel Cattle Company](https://www.kcattle.com/sale-bull-photos.html) | Broad muzzle, thick neck/shoulder transition, compact lower legs and substantial hindquarter mass. The minotaur interprets these features on a biped rig. |
| [Male back body, Robystarm07, CC BY-SA 4.0](https://commons.wikimedia.org/wiki/File:Male_back_body.jpg) | Rounded shoulder caps, narrowing waist and distinct posterior shoulder/back planes. Back and shoulder recipes follow these placements without using the photograph as geometry. |

[OpenStax, muscle attachments and fascicle arrangements](https://openstax.org/books/anatomy-and-physiology-2e/pages/11-1-interactions-of-skeletal-muscles-their-fascicle-arrangement-and-their-lever-systems)
provided anatomical context for anterior biceps/posterior triceps and convergent
pectoral bundles. Fantasy hybrids are original artistic interpretations; no
single real animal photograph validates a centaur or minotaur.

This is a procedural anatomical foundation. The photo review is qualitative,
not photogrammetry or a measured fit, and does not establish parity with a
high-end studio asset. Production work still needs artist review of proportions,
facial detail, skin/materials, corrective deformation and topology suitable for
the destination. The existing `avishape compare`/`fit` commands can measure
against a separately supplied licensed OBJ/GLB reference mesh.
