# CharMorph Android App

This is the Android application for CharMorph, a detailed character design tool.

## Architecture

The app follows a modular architecture:

- **app**: The main application module containing UI (Compose) and navigation.
- **core-model**: Shared data structures (`Mesh`, `Skeleton`, `MorphTarget`) and domain entities.
- **ingest-pipeline**: WorkManager tasks for importing and processing 3D assets.
- **ml-engine**: TensorFlow Lite wrappers and inference logic.
- **asset-base**: Manages built-in assets (base meshes, skeletons).
- **storage**: Room database for persisting character metadata and import history.
- **native-bridge**: JNI interface to C++ libraries (Assimp, Eigen) for heavy geometry processing.
- **preview-renderer**: 3D rendering module using **Google Filament**.
  - Provides `FilamentView` for high-fidelity PBR rendering.
- **feature-photo-import**: Module for "Photo to Character" functionality.
  - Uses ML Kit (Face Mesh) to detect landmarks.
  - Implements a parametric solver to map 2D landmarks to 3D morph sliders.

## Key Features

1.  **Detailed Morphing**: Inherits the Python core's logic (parametric modeling).
2.  **Real-time Preview**: Uses Filament for physically based rendering on mobile.
3.  **Photo Import**: "Rapid Photo to 3D" via landmark-based morph fitting.
4.  **Offline First**: All processing happens on-device (MediaTek target).

## Development Status

- [x] Module scaffolding
- [x] Core data models
- [x] Basic UI shell (Home, Import screens)
- [x] Ingestion pipeline worker stubs
- [x] **Filament Integration** (Renderer setup)
- [x] **Photo Import Module** (UI & Solver stub)
- [x] JNI/CMake setup
- [ ] TFLite model integration
- [ ] Full Mesh Parsing implementation

## References

- **MakeHuman**: The morphing system is inspired by MakeHuman's target-based approach.
- **Shape Atlas**: Anatomical accuracy is a key goal.
