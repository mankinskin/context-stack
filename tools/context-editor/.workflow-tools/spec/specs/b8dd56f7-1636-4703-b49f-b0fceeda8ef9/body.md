&lt;!-- aligned-structure:v2 --&gt;

# context-editor/sandbox/render

## Target Code Location

- [kernel/src/render/mod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/render/mod.rs) — render-graph plugin, `ContextEditorRenderPlugin`, `canvas_window_plugin`.
- [kernel/src/render/svo_ray_march.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/render/svo_ray_march.rs) — ray-march compute shader/uniforms.
- [kernel/src/render/depth_bridge.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/render/depth_bridge.rs), [kernel/src/render/glass.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/render/glass.rs), [kernel/src/render/ui_composite.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/render/ui_composite.rs), [kernel/src/render/wireframe_overlay.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/render/wireframe_overlay.rs), [kernel/src/render/runtime_params.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/render/runtime_params.rs) — render-graph node submodules.
- [kernel/src/render/pbr_material.wgsl](../../../../context-engine/context-stack/tools/context-editor/kernel/src/render/pbr_material.wgsl) — PBR/IBL shader.
- [kernel/src/splat/mod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/splat/mod.rs), [kernel/src/splat/force_compute.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/splat/force_compute.rs), [kernel/src/splat/particle_splat.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/splat/particle_splat.rs) — per-frame SVO-derived splat layer, treated as part of this render-graph contract per W1 (splat is "explicitly ephemeral/derived from SVO each frame," consumed inside the same `build_kernel_app` assembly as the render-graph plugin).

## Naming Conventions

- `ContextEditorRenderPlugin`, `canvas_window_plugin()` — crate-root re-export `render::runtime_params` (`pub use render::runtime_params;` in `lib.rs`).
- `ContextEditorLabel` — enum with one variant per render-graph stage, fixing node order.
- `ForceComputePlugin`, `ParticleSplatPlugin` — splat plugins, re-exported at crate root as `splat::{force_compute, particle_splat}` → `crate::force_compute::ForceComputePlugin` / `crate::particle_splat::ParticleSplatPlugin`.
- `VoxelSplat`, `SplatParams` — GPU structs for the splat layer.
- Criterion-id scheme: `context-editor-sandbox-render-<topic>` (e.g. `context-editor-sandbox-render-graph-node-order`).

## Requester Input

> The voxel world rendering is not yet correct. From certain viewing angles, artifacts still appear where voxels overlap incorrectly. It is partially possible to see into voxels. Lighting does not yet work properly. Voxel materials are not yet realistic enough. Many surfaces are still flat and monochrome. Reflective surfaces are too perfect.

## Reading Order

1. [context-editor/sandbox](../sandbox) — parent root, Shared Invariant 3 (fixed render-graph node order) governs this child directly.
2. [context-editor/sandbox/svo](../svo) — provider of leaf voxel data for ray-march traversal and splat regeneration.
3. [context-editor/sandbox/gpu-upload](../gpu-upload) — provider of BACK-buffer bind groups this render graph reads.
4. [ticket 8922e00c SVO Ray Marching epic](../../../../.workflow-tools/ticket/tickets/8922e00c-98ac-4604-ae01-29acca066b61/ticket.toml) — the epic whose implementation surface this child characterizes (extend relationship per W1 §5).

## Responsibility

`render` (together with `splat`, treated as one cohesive render-graph
contract per W1's own characterization) owns the custom Bevy render-graph
pipeline that turns SVO state into a displayed frame: `BufferSwap →
ParticleCompute → SvoRayMarch → DepthBridge → UiComposite →
WireframeOverlay`, plus the per-frame splat layer regenerated from SVO leaf
data. It does not own SVO mutation, GPU buffer allocation, or Dioxus UI
state — those belong to `svo`, `gpu-upload`, and `ui-bridge` respectively.

## Interfaces And Dependencies

- Depends on [svo](../svo) (leaf reads for ray-march traversal and splat regeneration) and [gpu-upload](../gpu-upload) (bind groups/buffers for the ray-march compute pass).
- Consumed by `build_kernel_app` via `render::canvas_window_plugin()`, `ContextEditorRenderPlugin`, and the crate-root splat re-exports.
- `world::theme::to_standard_material()` (see [world-materials](../world-materials)) writes into the same `Assets<StandardMaterial>` this render graph ultimately draws, though the write happens app-side (`sandbox-app/src/bootstrap.rs`), not inside this module.

## Behavior

The render-graph node order is fixed and structural, not incidental: `DepthBridge` explicitly depends on `SvoRayMarch`'s NDC-depth storage-buffer output, and `WireframeOverlay` draws last "with depth testing" against the composited result (per `render/mod.rs`'s own doc comment, characterized in W1 §2 item 2). The splat layer is regenerated every frame directly from SVO leaf iteration; it is explicitly ephemeral, not a cached or persisted representation.

## Boundaries And Failure Cases

- W1 identifies `render` as the single largest internal module boundary (6 render-graph node types plus 3 WGSL shader pairs) and flags `render::svo_ray_march`/`render::wireframe_overlay` as comparatively decoupled from `render::glass`/`ui_composite` — a candidate internal sub-boundary for a future restructure (W5), but no split is decided by this spec.
- `camera_query.single()` is used in at least one render/debug path; per [ARTIFACTS.md §5](../../../../transcripts/18-09-2026_context-editor-sandbox-architecture/ARTIFACTS.md), this can cause a silent no-draw condition if the query does not resolve to exactly one camera — flagged as a plausible root cause for reported viewing-angle artifacts, not yet confirmed against a runtime capture.
- Splat regeneration requires SVO leaf-iteration to be exposed as a stable internal API; W1 notes extracting `splat` to a separate crate "would require exporting `VoxelWorld` leaf-iteration as a stable public API, which does not exist today."

## Provider/Consumer Contract

- **Consumes from** [svo](../svo): leaf voxel/material data.
- **Consumes from** [gpu-upload](../gpu-upload): BACK-buffer bind groups (`DoubleBindGroups`).
- **Provides to** `sandbox-app` (outside this hierarchy, see parent Scope): the displayed frame via `canvas_window_plugin()`.

## Examples

A defect-fix session (W6) investigating "voxel overlap at certain viewing angles" reads this child's Boundaries section, finds the `camera_query.single()` candidate root cause already named with its source pointer, and can go straight to a capture-harness reproduction against that call site instead of re-deriving the candidate from scratch.

## Evidence

### Guards

- [`context-editor-kernel-native-tests`](../../../../.workflow-tools/test/specs/context-editor-kernel-native-tests.json) — `cargo test -p context-editor-kernel`; exercises 5 render-module test seams (`depth_bridge.rs`, `glass.rs`, `runtime_params.rs`, `svo_ray_march.rs`, `ui_composite.rs`) plus 3 splat test seams, all included in the 389-pass baseline.
- [`context-editor-kernel-wasm-check`](../../../../.workflow-tools/test/specs/context-editor-kernel-wasm-check.json) — `cargo check --target wasm32-unknown-unknown -p context-editor-kernel`; render/canvas.rs and render/mod.rs carry `#[cfg(target_arch = "wasm32")]` gates not exercised by the native run. Execution pending as of this spec's creation.

### Positions

- **implemented** — the fixed 6-stage render graph, PBR shader, splat layer (`code_ref`: [kernel/src/render/mod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/render/mod.rs), [kernel/src/splat/mod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/splat/mod.rs)).
- **not-implemented** — path tracing (multi-bounce), meshlet LOD, texture/normal-map/parallax material system (`code_ref`: none exists, confirmed absent per [ARTIFACTS.md §3](../../../../transcripts/18-09-2026_context-editor-sandbox-architecture/ARTIFACTS.md)). Do not present these as available anywhere this spec is introduced.
- **partial** — reflective-surface quality and lighting correctness are user-reported as defective (Requester Input above) but not yet reproduced against runtime capture evidence (`code_ref`: [kernel/src/render/pbr_material.wgsl](../../../../context-engine/context-stack/tools/context-editor/kernel/src/render/pbr_material.wgsl)).

### Governing-rule requirement

No PolicyRule introduces this spec in-session yet; out of scope for this waypoint.

## Scope

In scope: the fixed render-graph node order, ray-march traversal, splat
regeneration, and their SVO/GPU dependencies. Out of scope: GPU buffer
allocation itself (see [gpu-upload](../gpu-upload)), SVO mutation (see
[svo](../svo)), and any texture/normal-map/parallax system (not-implemented,
tracked by [ticket 9d754852](../../../../context-engine/context-stack/tools/context-editor/.workflow-tools/ticket/tickets/9d754852-42f7-49cd-a631-d399699ca9d0/ticket.toml)).