&lt;!-- aligned-structure:v2 --&gt;

# context-editor/sandbox/world-materials

## Target Code Location

- [kernel/src/world/theme.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/world/theme.rs) — `MaterialDef::to_standard_material()`, palette→`StandardMaterial` conversion.
- [kernel/src/world/svo_lod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/world/svo_lod.rs) — `SvoLodPlugin`, LOD color propagation.
- [kernel/src/world/world_gen.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/world/world_gen.rs) — `WorldGenPlugin`.
- [sandbox-app/src/bootstrap.rs](../../../../context-engine/context-stack/tools/context-editor/sandbox-app/src/bootstrap.rs) — `sync_palette_materials`, the app-side consumer of `to_standard_material()`.

## Naming Conventions

- `MaterialDef::to_standard_material()` — kernel-side conversion function; called by `sandbox-app/src/bootstrap.rs::sync_palette_materials` (app-side, line ~473 per W1).
- `SvoLodPlugin`, `WorldGenPlugin` — crate-root re-exports `world::{svo_lod, theme, world_gen}`.
- `propagate_lod_colors_single_level` — LOD color-propagation test function, reads `VoxelWorld` directly.
- Criterion-id scheme: `context-editor-sandbox-world-materials-<topic>` (e.g. `context-editor-sandbox-world-materials-palette-sync`).

## Requester Input

> Voxel materials are not yet realistic enough. Many surfaces are still flat and monochrome.

## Reading Order

1. [context-editor/sandbox](../sandbox) — parent root.
2. [context-editor/sandbox/svo](../svo) — provider of `VoxelWorld` LOD color data this module propagates.
3. [context-editor/sandbox/render](../render) — the consumer whose `pbr_material.wgsl` shader ultimately draws the `StandardMaterial` this module produces.
4. [ticket 947d0a30 Theme Palette Driving SVO Materials, PBR Parameters, and Glass Tints](../../../../.workflow-tools/ticket/tickets/947d0a30-1731-445e-a438-2eeab6b1c5bf/ticket.toml) — done ticket this module's palette contract reuses (W1 §5 "Reuse" classification).

## Responsibility

`world-materials` owns the world data layer's palette/LOD boundary: SVO LOD
color propagation (`svo_lod`), palette→`StandardMaterial`/PBR conversion
(`theme`), and world generation (`world_gen`). It provides the conversion
function the app-side bootstrap calls to keep displayed materials in sync
with the current palette; it does not own the shader that consumes the
resulting `StandardMaterial`, nor SVO storage itself.

## Interfaces And Dependencies

- Depends on [svo](../svo): `svo_lod` reads `VoxelWorld` directly for LOD color propagation.
- Consumed by `sandbox-app/src/bootstrap.rs::sync_palette_materials` (app-side, outside this hierarchy per the parent Scope), which queries `(&PaletteMesh, &MeshMaterial3d<StandardMaterial>)` and rewrites `Assets<StandardMaterial>` each time the palette changes.
- Feeds [render](../render): the `StandardMaterial` instances this module's conversion function produces are what `pbr_material.wgsl` ultimately shades.

## Behavior

`to_standard_material()` is a kernel→app boundary function, not an internal
kernel contract: the kernel provides the conversion; the app owns the Bevy
query/system that applies it (`sync_palette_materials`). This is a one-way
data flow — palette change in the kernel's theme data triggers an app-side
resync, not the reverse.

## Boundaries And Failure Cases

- Potential mismatches between palette values and shader behavior (e.g.
  `perceptual_roughness` vs `roughness` packing) are an unresolved evidence
  need per W1 — not a confirmed defect, but a plausible contributor to the
  reported "flat and monochrome" surfaces (Requester Input above).
- Because `sync_palette_materials` lives app-side, a change to this module's
  conversion function's output shape is a breaking change for `sandbox-app`
  specifically, not an internal-only kernel refactor — any restructure (W5)
  touching `theme.rs` must verify `sandbox-app/src/bootstrap.rs` still
  compiles and its query still matches the produced type shape.

## Provider/Consumer Contract

- **Consumes from** [svo](../svo): `VoxelWorld` LOD color data via `propagate_lod_colors_single_level`.
- **Provides to** `sandbox-app/src/bootstrap.rs::sync_palette_materials` (app-side): `MaterialDef::to_standard_material()`.
- **Provides to** [render](../render): the resulting `StandardMaterial` instances, shaded by `pbr_material.wgsl`.

## Examples

A defect-fix session (W6) investigating "voxel materials are not yet realistic enough / flat and monochrome" reads this child's Boundaries section, finds the `perceptual_roughness` vs `roughness` packing mismatch already named as the leading unresolved evidence need, and can prioritize capturing a runtime comparison of palette value vs shader input over inventing a new hypothesis.

## Evidence

### Guards

- [`context-editor-kernel-native-tests`](../../../../.workflow-tools/test/specs/context-editor-kernel-native-tests.json) — `cargo test -p context-editor-kernel`; exercises 3 world test seams (`svo_lod.rs`, `theme.rs`, `world_gen.rs`) plus `world/theme/tests.rs`'s direct exercise of `take_dirty_ranges()`, included in the 389-pass baseline.
- [`context-editor-sandbox-app-trunk-build`](../../../../.workflow-tools/test/specs/context-editor-sandbox-app-trunk-build.json) — `trunk build`; the guard that exercises `sandbox-app/src/bootstrap.rs::sync_palette_materials`, the app-side consumer this module's contract depends on staying compatible with. Execution pending as of this spec's creation.

### Positions

- **implemented** — palette→`StandardMaterial` conversion, LOD color propagation, world generation (`code_ref`: [kernel/src/world/theme.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/world/theme.rs), [kernel/src/world/svo_lod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/world/svo_lod.rs)).
- **not-implemented** — a texture/normal-map/parallax material system (`code_ref`: none exists, confirmed absent per [ARTIFACTS.md §3](../../../../transcripts/18-09-2026_context-editor-sandbox-architecture/ARTIFACTS.md); tracked as a Priority-2 defect-fix candidate by [ticket 9d754852](../../../../context-engine/context-stack/tools/context-editor/.workflow-tools/ticket/tickets/9d754852-42f7-49cd-a631-d399699ca9d0/ticket.toml), not by this spec).
- **partial** — palette/shader value-packing correctness (`perceptual_roughness` vs `roughness`) is an unresolved evidence need, not confirmed correct or defective (`code_ref`: [kernel/src/world/theme.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/world/theme.rs)).

### Governing-rule requirement

No PolicyRule introduces this spec in-session yet; out of scope for this waypoint.

## Scope

In scope: palette→material conversion, LOD color propagation, and world
generation. Out of scope: the shader that consumes the produced
`StandardMaterial` (see [render](../render)) and any new texture/normal-map/
parallax system (tracked by [ticket 9d754852](../../../../context-engine/context-stack/tools/context-editor/.workflow-tools/ticket/tickets/9d754852-42f7-49cd-a631-d399699ca9d0/ticket.toml), not this spec).