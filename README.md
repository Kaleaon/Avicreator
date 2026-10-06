# Avicreator Core Engine

Avicreator is a high-performance modular Rust workspace designed for cross-platform 3D avatar computation, mesh deformation, and morph blending.

## Workspace Architecture

The repository is structured into separate domain logic crates and target platform binding crates:

- **`crates/avicreator-schema`**: Serde-compatible data structures for asset manifests, skeleton hierarchies, and material specifications. Marked with `#![forbid(unsafe_code)]`.
- **`crates/avicreator-core`**: Pure Rust domain math, vector calculations (`Vec3`, `Quat`, `Mat4`), mesh operations, shape key morph blending, and vertex weight normalization. Marked with `#![forbid(unsafe_code)]`.
- **`crates/avicreator-wasm`**: WebAssembly API bindings exposed via `wasm-bindgen` and `web-sys` for browser execution.
- **`crates/avicreator-ffi`**: C-ABI function exports and C header generation via `cbindgen` for C/C++ native, iOS, and Android mobile bridge integration.

## Build & Test Instructions

### Workspace Build & Test
```bash
# Build all workspace members
cargo build --workspace

# Run unit test suites
cargo test --workspace
```

### WebAssembly Compilation
```bash
cargo build -p avicreator-wasm --target wasm32-unknown-unknown --release
```

### C-FFI & Header Generation
Building `avicreator-ffi` automatically generates `crates/avicreator-ffi/include/avicreator.h` using `cbindgen`:
```bash
cargo build -p avicreator-ffi
```

## Security & Memory Management
- Pure domain crates (`avicreator-schema`, `avicreator-core`) strictly enforce `#![forbid(unsafe_code)]`.
- The C-FFI layer provides explicit buffer deallocation helper functions (`avicreator_free_string`, `avicreator_free_f32_buffer`, `avicreator_free_u32_buffer`, `avicreator_engine_destroy`) to prevent cross-language memory leaks.
