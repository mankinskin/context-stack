# Context Editor Sandbox App

This project hosts the `context-editor-kernel` in a browser or a native desktop window. It combines a Bevy-based voxel/render engine with a Dioxus UI shell.

## Overview

The sandbox app is the entry point for the context editor toolchain. It:

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

## Run natively (Linux / WSL2)

From the repository root:

```bash
cargo run -p sandbox-app
```

The scene and Dioxus controls share one Bevy window. The native host renders the
UI with Blitz and Vello CPU into a transparent texture; the 3D scene uses Bevy's
GPU renderer. No browser, Trunk, GTK, or WebKit is required.

On Ubuntu, install the window-system dependencies:

```bash
sudo apt-get update
sudo apt-get install -y libwayland-dev libxkbcommon-dev libxkbcommon-x11-0
```

WSL2 also needs a working WSLg session. `DISPLAY` and/or `WAYLAND_DISPLAY` must
point to a reachable display, with `XDG_RUNTIME_DIR` configured for Wayland.
Restart the WSL session after enabling WSLg. Native builds support both Wayland
and X11.

- Use WASD / arrow keys to move, hold the left mouse button in the scene to look
  around, and Shift to sprint.
- UI panels capture their mouse input; typing into the terminal does not move
  the camera. Click back into the scene to return keyboard control.
- The same debug settings, presets, theme switch (`T`), and terminal callbacks
  are available. Native numeric controls use `-` / `+` buttons instead of
  browser range sliders.
- Native styling is bundled locally and does not require the Tailwind CDN.
- Check the startup `AdapterInfo` log: `llvmpipe` means software rendering and
  can be very slow. WSLg display support alone does not guarantee a
  hardware-accelerated Vulkan adapter.

## Run in the browser

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

For the native UI adapter (including headless input and texture rendering tests):

```bash
cargo test -p context-editor-kernel --features native-ui --lib
```

## Visual verification

The project includes a deterministic visual baseline check. From the `context-engine` root:

```bash
node context-stack/tools/context-editor/sandbox-app/e2e/verify-visual-baseline.mjs
```

This is used to compare the current browser-rendered output against the recorded baseline, and it is especially useful for confirming render regressions or defect fixes.

## Notes

- Browser builds target `wasm32`; native builds enable the kernel's `native-ui` feature automatically.
- Some headless or constrained environments cannot initialize WebGPU, which can prevent visual validation from running in the same way as a full desktop/browser setup.
- The sandbox is designed for demonstration and validation, not as the final production editor surface.

## Related code

- `context-editor-kernel` — render, SVO, UI, editor, and simulation layers
- `sandbox-app` — web-hosted integration and world bootstrap layer
