# Android Ingestion App Plan

This document captures the current architecture for the Kotlin-only CharMorph ingestion app.
Target hardware: mid-tier MediaTek devices with 4 GB RAM.

## Vision

Deliver a portable character customiser inspired by Spore’s creature editor. Users upload meshes (zip/folders), the app fits CharMorph base meshes, and produces sliders for skin/muscle/fat deformation – all on-device.

## Pillars

- **Kotlin-first**: Compose UI, coroutines, WorkManager, Room.
- **Deterministic ML**: TensorFlow Lite (FP16/INT8) for geometry fitting and envelope prediction.
- **Resource-aware**: stream data, run jobs sequentially, release native buffers quickly.
- **Spore-flavoured UX**: playful slider UI, preview of muscle/fat layers, procedural suggestions.

## Module Layout

| Module | Responsibility |
| --- | --- |
| `app` | Compose UI, navigation, DI wiring |
| `core-model` | Shared data classes, serialization, logging |
| `asset-base` | Bundled XML base meshes, skeleton references |
| `ingest-pipeline` | WorkManager tasks, orchestrator |
| `ml-engine` | TFLite model loading + inference helpers |
| `native-bridge` | JNI wrappers (Assimp, Eigen math, optional ONNX runtime) |
| `storage` | Room DB, cache manager, cleanup scheduler |

## Processing Pipeline

1. **Import**
   - Storage Access Framework; accept single files or zips.
   - Copy into sandboxed staging directory (`Context.cacheDir/uploads/<session>`).

2. **Unpack & Parse**
   - Native `AssimpImporter` JNI call returns topology, rig, materials metadata.
   - Streaming approach: parse mesh by mesh to avoid high RAM bursts.

3. **Feature Extraction (JNI)**
   - Compute vertex normals, Laplacian coordinates, bone distances, envelope hints.
   - Build tensors for ML models, stored in direct byte buffers.

4. **ML Fitting**
   - **Model A** (Graph Conv, ~5 MB): warps base mesh to match uploaded geometry.
   - **Model B** (MLP, <1 MB): predicts skin/muscle/fat distribution per vertex/bone.
   - TFLite, FP16/int8 quantised, run on CPU with 2–3 threads max.

5. **Slider Synthesis**
   - Kotlin logic groups weights by region/bone, generates slider definitions (min/max/default).
   - Compose-friendly JSON stored via Room.

6. **Result Persistence**
   - Cache metadata + preview textures in `Context.filesDir/results/<session>`.
   - Option to export as zipped JSON + glTF.

7. **Cleanup**
   - Post-processing worker clears staging directory, respects user retention settings.

## WorkManager Flow

```
ImportRequest (one-off):
  - enqueue SequentialChain(
      PrepareUploadsWorker,
      ParseMeshWorker,
      FeatureExtractionWorker,
      MLFittingWorker,
      SliderSynthesisWorker,
      FinalizeWorker)
```

Each worker produces a small JSON state blob in Room (or DataStore) to resume if the job is interrupted.

## UI Sketch

- **Home**: queue list + last results.
- **Import**: button to pick file/folder; shows accepted formats.
- **Processing detail**: progress indicator, logs, estimated time.
- **Creature view**: Spore-like preview with layered toggles (Skin, Muscle, Fat), slider grid, export/share actions.

## Asset Strategy

- Bundle `HumanoidNeutral.xml`, `HumanoidAthletic.xml`, plus skeletal references.
- Allow optional “resource packs”: zipped additive assets downloaded or sideloaded. Drop into `Android/data/.../resources/`.
- Document zipped bundle structure (`/models`, `/skeletons`, `/weights`, metadata JSON).

### Skeleton Baseline

- Adopt the standard Second Life (SL) skeleton as the canonical rig.
- Convert SL joint definitions, constraints, and naming to our XML format and export as a Kotlin data snapshot for fast lookup.
- Provide compatibility layers:
  - `sl_rig.json` → default, animation-ready rig.
  - `sl_extended.json` → additional bones / helper joints for muscle/fat simulation.
- During ingestion, map uploaded rigs onto SL bones (name matching + fallback heuristics).
- Ensure generated sliders respect SL bone limits (joint rotations, stretch factors) to guarantee export compatibility with existing animation packs.

### Soft Tissue Layers

- **Skin weights**:
  - Use MPFB2 vertex groups as initial ground truth.
  - Blend in SL skin envelope data where available to ensure compatibility with existing clothing systems.
  - Generate fallback weights using Laplacian-based projection when imported models lack rigging.
- **Muscle & fat wraps**:
  - Leverage reference meshes from MPFB2 plus anatomical datasets to define “muscle” and “fat” envelopes.
  - Train TFLite predictors to output per-bone density maps; quantize to fp16/int8 targets.
- **Tendon/volume preservation**:
  - Introduce helper bones (from `sl_extended.json`) to capture bulges and joint-dependent stretching.
  - Ensure sliders drive both shape morphs and weight redistribution for natural deformation.

### Muscle Deformer System

- **Hyper-muscle morph targets**:
  - Generate layered morphs (base → athletic → hyper) from MPFB2 muscular presets and custom sculpts.
  - Store as high-resolution displacement vectors, simplified into multi-resolution deltas for low-memory devices.
- **Procedural muscle inflators**:
  - Implement Kotlin-side modifiers that scale specific muscle groups based on slider values (e.g., biceps, deltoids).
  - Use TFLite predictors to compute bulge magnitude given bone angles and user-defined “hyper” sliders.
  - Blend shape + pose-dependent corrections so extreme poses maintain believable volume.
- **Dynamic deformers (optional)**:
  - Attach secondary simulation to helper bones (spring-mass or position-based dynamics) for jiggle and flex.
  - Provide toggle in UI for users who want purely baked morphs vs dynamic runtime effects.
- **Performance safeguards**:
  - Clamp extreme sliders to physically plausible ranges unless “pro mode” enabled.
  - Cache deformer outputs per pose frame to reduce repeated ML inference.

### Biomechanics & Validation (OpenSim)

- Integrate datasets from [OpenSim](https://opensim.stanford.edu/) to validate joint ranges, muscle volumes, and center-of-mass changes.
- Use OpenSim-derived muscle paths and moment arms to calibrate our muscle/fat weight distributions.
- Where licensing permits, convert OpenSim musculoskeletal models into simplified training targets (resampled to SL skeleton) for the ML pipeline.
- Implement offline validation scripts that compare our generated muscle/fat envelopes against OpenSim references, enforcing error thresholds before assets ship.

## ML Considerations

- Use offline training to convert skeleton examples into TFLite models.
- Introduce fallback deterministic heuristics if ML assets unavailable (ensures minimal functionality).
- Provide telemetry toggles for opt-in performance stats (for iterative tuning).

## Next Steps

1. Scaffold a Gradle multi-module project inside `/android`.
2. Implement JNI proof-of-concept: load `.obj`, read vertex count, return to Kotlin.
3. Define TFLite model interfaces (`GeometryFitter`, `WeightPredictor`).
4. Capture sample skeleton meshes/muscle wraps for training dataset.

## Open Questions

- Should we support sculpting-inspired live manipulations (real-time)?
- How to expose procedural suggestions (Spore DNA buds) without ML drift?
- Do we need partial body uploads (limb-only) and grafting support?
