&lt;!-- aligned-structure:v2 --&gt;

# context-editor/sandbox/svo

## Target Code Location

- [kernel/src/svo/mod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/svo/mod.rs) — `VoxelWorld` resource, manipulation API, dirty-range tracking.
- [kernel/src/svo/paging.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/svo/paging.rs) — paged SVO layout.
- [kernel/src/svo/upload.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/svo/upload.rs) — `SvoUploadPlugin`, per-frame dirty-range drain.
- [kernel/src/svo/tests.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/svo/tests.rs) — packing, set/remove, brush coverage tests.

## Naming Conventions

- `VoxelWorld` — the SVO resource; doc comment states it is "the single source of truth for world geometry."
- `OctreeNode`, `VoxelMaterial` — node/material types, re-exported at `svo::{OctreeNode, VoxelMaterial}` via an internal `pub use types::{...}` inside `svo/mod.rs` (not re-exported at crate root).
- `take_dirty_ranges()` — drain method; the SVO upload/double-buffer contract's regression guard (see Shared Invariants on the [parent spec](../sandbox)).
- `apply_sdf_brush()` / `carve_sdf_brush()` — mutation entry points, defined here and called by the [editor child](../editor).
- `SvoUploadPlugin` — used directly via full path (`svo::upload::SvoUploadPlugin`) in `build_kernel_app`, not re-exported at crate root.
- Criterion-id scheme: `context-editor-sandbox-svo-<topic>` (e.g. `context-editor-sandbox-svo-dirty-range-drain`).

## Requester Input

> The game should support raytracing graphics, signed distance fields, meshlet LOD, mesh signed distance fields, and octree optimization.

## Reading Order

1. [context-editor/sandbox](../sandbox) — parent root, Shared Invariants 1-2 govern this child directly.
2. [context-editor/sandbox/render](../render) — the primary consumer of SVO leaf data via ray-march traversal.
3. [context-editor/sandbox/gpu-upload](../gpu-upload) — the provider `svo` hands dirty ranges to once per frame.
4. [context-editor/sandbox/editor](../editor) — the consumer that mutates `VoxelWorld` via SDF brushes.
5. [ticket 7205c1e1 part b7664292 §1 (svo row)](../../../../context-engine/context-stack/tools/context-editor/.workflow-tools/ticket/tickets/7205c1e1-273e-46f8-afd1-bf599a50e344/parts/b7664292-1ede-4938-b34c-3f5b2878e14b.md) — W1's classification of `svo` as "the highest-risk extraction candidate to reject."

## Responsibility

`svo` owns the sparse voxel octree as the kernel's single geometry authority:
node storage (`OctreeNode`), material tagging (`VoxelMaterial`), paged layout
(`paging`), mutation with interior-flag reclassification (`set_voxel`/
`remove_voxel`), and dirty-range tracking consumed once per frame by the GPU
upload path. It does not render, does not collide, and does not own any
Bevy-facing plugin registration for consumers — those live in `render`,
`physics`, and `gpu-upload`.

## Interfaces And Dependencies

- Depended on by `physics` (collision authority — out of this hierarchy's scope, see parent Scope), `splat` (per-frame leaf read, part of the [render child](../render)), `render` (ray-march traversal), `gpu-upload` (dirty-range drain target), `editor` (brush mutation), and `world` (LOD color propagation, part of the [world-materials child](../world-materials)). It is, per W1, "the most fanned-in module in the kernel."
- Depends on no other kernel module; it is the structural leaf of the dependency graph.

## Behavior

`set_voxel`/`remove_voxel` mutate a node and reclassify neighbor interior
flags; each mutation marks the affected page range dirty and sets a
color-propagation flag. `take_dirty_ranges()` (`svo/mod.rs:223` per W1) drains
and returns the accumulated dirty ranges; callers (only `svo::upload`'s
`SvoUploadPlugin`, `upload.rs:175`) must invoke it exactly once per frame —
calling it twice in one frame silently starves the second caller, since the
first drain empties the range set (asserted by `svo/upload.rs`'s own unit
tests at lines ~258-280).

## Boundaries And Failure Cases

- Draining `take_dirty_ranges()` more than once per frame from a second
  caller (e.g. a future capture harness) returns an empty range set for that
  second caller, not an error — this is a silent starvation failure mode, not
  a panic, and any new consumer must coordinate with `svo::upload`'s existing
  single-drain call site rather than adding a second one.
- `set_voxel`/`remove_voxel` on an out-of-bounds coordinate is not
  characterized by W1; this pass does not assert a defined boundary behavior
  here and flags it as an evidence gap for a future characterization pass.

## Provider/Consumer Contract

- **Provides to** [render](../render): leaf voxel/material data for ray-march traversal and splat regeneration.
- **Provides to** [gpu-upload](../gpu-upload): dirty page ranges via `take_dirty_ranges()`, drained exactly once per frame.
- **Consumes from** [editor](../editor): brush mutation calls (`apply_sdf_brush`/`carve_sdf_brush`) that this module executes against `VoxelWorld`.

## Examples

A restructure session considering whether `svo` can be split into its own
crate reads this child and W1's row directly: every other load-bearing child
(`render`, `gpu-upload`, `editor`, `world-materials`) depends on `VoxelWorld`
directly, so extracting `svo` alone "would just relocate the fan-in, not
reduce coupling" (W1's own conclusion) — the correct action is to treat `svo`
as the stable kernel core in any restructure, not a boundary to split.

## Evidence

### Guards

- [`context-editor-kernel-native-tests`](../../../../.workflow-tools/test/specs/context-editor-kernel-native-tests.json) — `cargo test -p context-editor-kernel`; last confirmed run: 389 passed / 0 failed (native target), per [ticket 7205c1e1 part b7664292 §4](../../../../context-engine/context-stack/tools/context-editor/.workflow-tools/ticket/tickets/7205c1e1-273e-46f8-afd1-bf599a50e344/parts/b7664292-1ede-4938-b34c-3f5b2878e14b.md).
- [`context-editor-kernel-wasm-check`](../../../../.workflow-tools/test/specs/context-editor-kernel-wasm-check.json) — `cargo check --target wasm32-unknown-unknown -p context-editor-kernel`; execution pending as of this spec's creation (spec created this pass, not yet run).

### Positions

- **implemented** — `VoxelWorld` mutation API, dirty-range tracking, paging (`code_ref`: [kernel/src/svo/mod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/svo/mod.rs), [kernel/src/svo/paging.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/svo/paging.rs)).
- **not-implemented** — mesh extraction/marching cubes/meshlets from SVO data (`code_ref`: none exists, confirmed absent per [ARTIFACTS.md §3](../../../../transcripts/18-09-2026_context-editor-sandbox-architecture/ARTIFACTS.md)).

### Governing-rule requirement

No PolicyRule introduces this spec in-session yet; creating one is out of
scope for this single-session waypoint (spec authoring only).

## Scope

In scope: `VoxelWorld`, node/material types, paging, mutation, and the dirty-
range drain contract. Out of scope: rendering (see [render](../render)),
GPU buffer layout (see [gpu-upload](../gpu-upload)), and brush-tool UX (see
[editor](../editor)).