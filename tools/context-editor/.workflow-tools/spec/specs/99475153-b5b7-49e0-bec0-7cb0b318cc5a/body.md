&lt;!-- aligned-structure:v2 --&gt;

# context-editor/sandbox/editor

## Target Code Location

- [kernel/src/editor/core.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/editor/core.rs) — `EditorPlugin`, brush mutation entry points.
- [kernel/src/editor/sdf_cutting.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/editor/sdf_cutting.rs) — SDF cutting.
- [kernel/src/editor/advanced_tools.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/editor/advanced_tools.rs), [kernel/src/editor/debug_overlay.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/editor/debug_overlay.rs), [kernel/src/editor/ux.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/editor/ux.rs) — advanced tools, wireframe/debug overlay, undo/redo/symmetry UX.

## Naming Conventions

- `EditorPlugin`, `AdvancedToolsPlugin`, `SdfCuttingPlugin`, `EditorUxPlugin`, `DebugOverlayPlugin` — plugins registered directly in `build_kernel_app`, re-exported at crate root as `editor::{advanced_tools, debug_overlay, sdf_cutting, editor_ux}`.
- `apply_sdf_brush()` / `carve_sdf_brush()` — defined on [svo](../svo)'s `VoxelWorld`, called from this module's `core.rs`.
- **Removable debt**: `editor/mod.rs` wildcard-re-exports `core::*` and `ux::*` internally — W1 classifies this specific wildcard layer (distinct from the named crate-root re-exports) as "explicitly a backward-compat shim per its own doc comment," retained only "so `crate::editor::Foo` continues to resolve as before."
- Criterion-id scheme: `context-editor-sandbox-editor-<topic>` (e.g. `context-editor-sandbox-editor-brush-mutation`).

## Requester Input

> Add voxel editing. ... Apply the same pattern to voxels so individual voxels can be edited, including their material. Use that later as the basis for a tool that can place or remove voxels.

## Reading Order

1. [context-editor/sandbox](../sandbox) — parent root.
2. [context-editor/sandbox/svo](../svo) — provider of `VoxelWorld` and the brush mutation entry points this module calls.
3. [ticket 6d7ddc8e World Editor: Voxel Paint/Carve with Live Splat Regeneration](../../../../.workflow-tools/ticket/tickets/6d7ddc8e-ba62-4ee9-9358-9d6d0642ab1e/ticket.toml) — done ticket this module's current paint/carve behavior reuses (W1 §5 "Reuse" classification).
4. [ticket e4ff45b1 feasibility gate](../../../../context-engine/context-stack/tools/context-editor/.workflow-tools/ticket/tickets/e4ff45b1-996c-4de5-889c-d694c757708c/ticket.toml) — gates any stretch-goal expansion of this module (individual-voxel editing, place/remove tooling) named in the Requester Input above.

## Responsibility

`editor` owns the voxel paint/carve/undo/redo/symmetry authoring surface:
SDF-brush mutation dispatch (`core.rs`), SDF cutting, an advanced-tools
plugin set, a wireframe/debug overlay, and undo/redo/symmetry UX. It mutates
[svo](../svo)'s `VoxelWorld` directly via `apply_sdf_brush`/`carve_sdf_brush`
but owns no SVO storage itself.

## Interfaces And Dependencies

- Depends on [svo](../svo)'s `VoxelWorld` and its brush mutation API.
- Consumed by `build_kernel_app` via 5 plugins (`EditorPlugin`, `AdvancedToolsPlugin`, `SdfCuttingPlugin`, `EditorUxPlugin`, `DebugOverlayPlugin`).
- Consumed by [ui-bridge](../ui-bridge): `ui/mod.rs` imports `crate::editor::debug_overlay::DebugPanel` directly for the Dioxus debug panel.

## Behavior

Editing operations call `apply_sdf_brush`/`carve_sdf_brush` on `VoxelWorld`,
which internally marks dirty ranges and reclassifies neighbor interior flags
(see the [svo child](../svo)'s Behavior section). The wireframe/debug overlay
draws using data these mutations produce; per W1, wireframe upload "sets
`index_count = 0` when its prerequisites fail," a silent no-draw path rather
than a visible error.

## Boundaries And Failure Cases

- The `editor/mod.rs` wildcard re-export (`pub use core::*` / `ux::*`) is
  explicitly removable debt per its own doc comment; a future restructure
  (W5) can retire it without behavior change since the named crate-root
  re-exports already cover direct submodule access — this spec does not
  itself retire it (out of scope, see Scope below).
- Wireframe overlay's `index_count = 0` fallback (see Behavior) is the most
  likely current-code candidate for the reported "Wireframe Grid... may not
  appear or may not be sized correctly" defect (Requester Input on the
  [render child](../render) covers the broader visual-defect list); this
  spec flags the candidate but does not assert it as the confirmed root
  cause absent a capture-harness reproduction.

## Provider/Consumer Contract

- **Consumes from** [svo](../svo): `apply_sdf_brush`/`carve_sdf_brush` and the `VoxelWorld` resource.
- **Provides to** [ui-bridge](../ui-bridge): `crate::editor::debug_overlay::DebugPanel`, imported directly by `ui/mod.rs`.

## Examples

A future stretch-prototype session (W7, gated by [ticket e4ff45b1](../../../../context-engine/context-stack/tools/context-editor/.workflow-tools/ticket/tickets/e4ff45b1-996c-4de5-889c-d694c757708c/ticket.toml)) evaluating "editing individual voxels including their material" reads this child, finds the existing brush-mutation entry points already documented with their [svo](../svo) dependency, and can scope a feasibility verdict against real interfaces instead of re-deriving them from source.

## Evidence

### Guards

- [`context-editor-kernel-native-tests`](../../../../.workflow-tools/test/specs/context-editor-kernel-native-tests.json) — `cargo test -p context-editor-kernel`; exercises 5 editor test seams (`advanced_tools.rs`, `core.rs`, `debug_overlay.rs`, `sdf_cutting.rs`, `ux.rs` — the largest test-seam count of any module per W1), included in the 389-pass baseline.
- [`context-editor-kernel-wasm-check`](../../../../.workflow-tools/test/specs/context-editor-kernel-wasm-check.json) — `cargo check --target wasm32-unknown-unknown -p context-editor-kernel`; `editor/debug_overlay.rs` carries a `#[cfg(target_arch = "wasm32")]` gate not exercised by the native run. Execution pending as of this spec's creation.

### Positions

- **implemented** — SDF brush paint/carve, undo/redo/symmetry UX, debug overlay (`code_ref`: [kernel/src/editor/core.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/editor/core.rs), [kernel/src/editor/ux.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/editor/ux.rs)).
- **not-implemented** — per-voxel individual-material editing UI, place/remove voxel tooling as a first-class user tool (`code_ref`: none; the underlying brush primitives exist but no dedicated UI wires them per-voxel, confirmed by comparing this module's API surface against the [ui-bridge child](../ui-bridge)'s stubbed sidebar/inventory panels).

### Governing-rule requirement

No PolicyRule introduces this spec in-session yet; out of scope for this waypoint.

## Scope

In scope: brush mutation dispatch, SDF cutting, undo/redo/symmetry, and the
debug/wireframe overlay. Out of scope: SVO storage itself (see
[svo](../svo)) and any new per-voxel editing UI (stretch-goal, gated by
[ticket e4ff45b1](../../../../context-engine/context-stack/tools/context-editor/.workflow-tools/ticket/tickets/e4ff45b1-996c-4de5-889c-d694c757708c/ticket.toml)).