# Context Editor Sandbox App

This project is a browser-hosted sandbox for the `context-editor-kernel` crate. It combines a Bevy-based voxel/render engine with a Dioxus UI shell so the editor kernel can be demonstrated, exercised, and validated in a web environment.

## Overview

The sandbox app is the WASM entry point for the context editor toolchain. It:

- loads the kernel runtime and world implementation
- registers voxel world presets and startup behavior
- hosts the Bevy canvas and UI shell in the browser
- serves as the validation target for visual regression and build checks

The implementation is intentionally small and acts as a host for the more substantial kernel code under:

- `../kernel`
- `../kernel/src`

## Project structure

- `src/` — sandbox-specific app and world wiring
- `index.html` — HTML shell and browser setup
- `Trunk.toml` — Trunk configuration for local serving and build
- `e2e/` — visual validation and browser fixtures
- `Cargo.toml` — Rust package definition and runtime dependencies

## Requirements

- Rust toolchain
- Cargo
- Trunk (`cargo install trunk` or the repository's usual tool bootstrap)
- A browser with GPU/WebGPU support for visual rendering checks

## Run locally

From this directory:

```bash
trunk serve
```

This starts the app in development mode on the configured local port (default Trunk port is typically `8080` unless overridden in configuration).

## Build

```bash
trunk build --release
```

## Validate the kernel

From the `context-engine` root:

```bash
cargo test -p context-editor-kernel
```

## Visual verification

The project includes a deterministic visual baseline check. From the `context-engine` root:

```bash
node context-stack/tools/context-editor/sandbox-app/e2e/verify-visual-baseline.mjs
```

This is used to compare the current browser-rendered output against the recorded baseline, and it is especially useful for confirming render regressions or defect fixes.

## Notes

- The app targets `wasm32` and uses the browser runtime for the Bevy render loop.
- Some headless or constrained environments cannot initialize WebGPU, which can prevent visual validation from running in the same way as a full desktop/browser setup.
- The sandbox is designed for demonstration and validation, not as the final production editor surface.

## Related code

- `context-editor-kernel` — render, SVO, UI, editor, and simulation layers
- `sandbox-app` — web-hosted integration and world bootstrap layer
