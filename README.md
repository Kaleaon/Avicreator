# Avicreator

Avicreator is the primary repository for avatar development. It contains the
CharMorph-based Blender addon, model-ingestion API, and Android application
imported from [Kaleaon/Avatarmaker](https://github.com/Kaleaon/Avatarmaker).
The import preserves the original Git history and license notices.

## Develop

Use Python 3.12 and Git. From this checkout:

```sh
bash scripts/setup.sh
bash scripts/dev.sh test
bash scripts/dev.sh serve
```

Setup creates an ignored `.venv`, installs pinned Python dependencies, and
initializes the character database and its nested asset submodules. The initial
asset download is large. The API binds to `127.0.0.1:8000` by default; use
`bash scripts/dev.sh serve --port 8001` to select another port.

Check `/health` for `{"status":"ok"}` and `/base-meshes` for available presets.
Upload a model to `/ingest-model` using multipart field `files` and
`base_mesh_id=HumanoidNeutral`. API documentation is served at `/docs`.

## Components

- Root Python modules, `lib/`, and `cmedit/`: Blender addon and avatar processing.
- `webapp/`: FastAPI model-ingestion service.
- `android/`: native Android avatar editor.
- `data/`: pinned CharMorph database submodule; retain per-character licenses.
- `tests/`: headless Python tests with mocked Blender APIs.
- [Original documentation](docs/AVATARMAKER.md): addon installation and attribution.

[Ktheme](https://github.com/Kaleaon/Ktheme) remains a separate theme-engine
repository. Keep its checkout beside Avicreator for integrations; the Android
project already includes local Ktheme library modules.

## Android development

Use a complete JDK 17, Android SDK 34, build-tools 34.0.0, NDK 26.1.10909125,
and CMake 3.22.1. The wrapper pins Gradle 8.8:

```sh
cd android
bash gradlew assembleDebug test --max-workers=2
```

Cloud Java downloads must use the platform's HTTPS proxy and managed CA trust
store. Android unit tests also need a writable Robolectric cache. The cloud
environment startup instructions describe those local overrides.

The imported full Android build currently fails in `FilamentController.kt` on
`morphTargetCount`, `morphTargetBuffer`, and `destroyMorphTargetBuffer`, which
are unavailable in the pinned Filament 1.32.0 API. This migration preserves that
known source issue. Python/API development is independently usable.

## Licensing

Preserve [license.txt](license.txt), [gpl-3.0.txt](gpl-3.0.txt), and
[agpl-3.0.txt](agpl-3.0.txt). Imported Python code is GPL-3.0-or-later;
database assets have their own licenses. Consult their notices before reusing
assets or generated models.
