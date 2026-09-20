&lt;!-- aligned-structure:v2 --&gt;

# context-editor/sandbox/gpu-upload

## Target Code Location

- [kernel/src/gpu/mod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/gpu/mod.rs) — `SvoPageTableBuffer`, `SvoTransformBuffer`, `DoubleBindGroups`, front/back SVO double-buffer resources.
- [kernel/src/gpu/svo_transform.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/gpu/svo_transform.rs) — transform buffer, inline `mod tests`.
- [kernel/src/svo/upload.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/svo/upload.rs) — `SvoUploadPlugin`, the single caller of `take_dirty_ranges()`.

## Naming Conventions

- `SvoPageTableBuffer`, `SvoTransformBuffer` — GPU-side buffer resources.
- `DoubleBindGroups` — front/back bind-group pair; the swap primitive the [render child](../render) reads.
- `SvoUploadPlugin` — the Bevy plugin that drains SVO dirty ranges and writes the BACK buffer.
- No `lib.rs` crate-root re-export exists for `gpu::*`; callers must use `crate::gpu::...` directly (W1: "structural, no compatibility shim").
- Criterion-id scheme: `context-editor-sandbox-gpu-upload-<topic>` (e.g. `context-editor-sandbox-gpu-upload-double-buffer-swap`).

## Reading Order

1. [context-editor/sandbox](../sandbox) — parent root, Shared Invariant 2 (once-per-frame dirty-range drain) governs this child directly.
2. [context-editor/sandbox/svo](../svo) — provider of the dirty ranges this module drains and uploads.
3. [context-editor/sandbox/render](../render) — consumer of the `DoubleBindGroups` this module produces.

## Responsibility

`gpu-upload` owns the GPU buffer/bind-group infrastructure that bridges SVO
mutation to the render graph: it drains dirty page ranges from `svo` exactly
once per frame via `SvoUploadPlugin`, writes them into the BACK buffer of a
double-buffered bind-group pair, and swaps front/back so `render` always
reads a stable, fully-written buffer. It owns no rendering logic and no SVO
mutation logic itself.

## Interfaces And Dependencies

- Consumed by [render](../render) (ray-march bind groups) and by `svo::upload` (writes the BACK buffer) — the latter is technically the same crate boundary (`svo/upload.rs` lives under `svo`, but its target is this module's `DoubleBindGroups`), so the [svo child](../svo) and this child jointly own the upload contract: `svo` owns the drain call site, `gpu-upload` owns the buffer/bind-group resources being written.
- No other kernel module reads or writes `gpu`'s buffers directly.

## Behavior

The swap doc comment at `gpu/mod.rs` states the front/back swap completes in
"< 0.01 ms." `SvoUploadPlugin` (`svo/upload.rs:175`) drains dirty ranges via
`take_dirty_ranges()` and writes them into the BACK buffer; the swap then
makes that buffer the new FRONT for `render` to read on the next frame. This
is a single-writer, single-reader-per-frame contract: exactly one plugin
drains and writes, exactly one render stage (`BufferSwap`, the first
render-graph node per the [render child](../render)) triggers the swap.

## Boundaries And Failure Cases

- A second caller draining `take_dirty_ranges()` in the same frame (see the [svo child](../svo)'s Boundaries section) starves this module's own upload write for that frame — the BACK buffer would receive an empty range set, silently. No new consumer of `take_dirty_ranges()` may be added without routing through this existing single-drain call site.
- W1 found no `lib.rs` re-export of `gpu::*`, so any future public API change to this module is a breaking API change for `svo::upload`'s internal caller, not merely an internal refactor — the module boundary is real, not a compatibility shim.

## Provider/Consumer Contract

- **Consumes from** [svo](../svo): dirty page ranges via the `take_dirty_ranges()` drain, exactly once per frame.
- **Provides to** [render](../render): `DoubleBindGroups` (front/back SVO buffer bind groups) for the ray-march compute pass.

## Examples

A future capture harness (W3) that wants deterministic GPU-buffer snapshots must trigger the exact `take_dirty_ranges()` → BACK-buffer-write → swap sequence documented here in one call, matching the existing single-writer contract, rather than reading buffer state at an arbitrary point in the frame.

## Evidence

### Guards

- [`context-editor-kernel-native-tests`](../../../../.workflow-tools/test/specs/context-editor-kernel-native-tests.json) — `cargo test -p context-editor-kernel`; exercises `gpu/mod.rs` (near line 384) and `gpu/svo_transform.rs` inline `mod tests`, included in the 389-pass baseline.
- [`context-editor-kernel-wasm-check`](../../../../.workflow-tools/test/specs/context-editor-kernel-wasm-check.json) — `cargo check --target wasm32-unknown-unknown -p context-editor-kernel`; execution pending as of this spec's creation.
- [`context-editor-sandbox-app-trunk-build`](../../../../.workflow-tools/test/specs/context-editor-sandbox-app-trunk-build.json) — `trunk build`; confirms the wasm32 build path that actually exercises this module's GPU buffer wiring end to end. Execution pending as of this spec's creation.

### Positions

- **implemented** — `SvoPageTableBuffer`, `SvoTransformBuffer`, `DoubleBindGroups`, and the front/back swap (`code_ref`: [kernel/src/gpu/mod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/gpu/mod.rs)).
- **partial** — wasm/native parity for this module's buffer wiring is unverified; the native test run exercised the resource types but not the wasm32-only `build_kernel_app` assembly path (`code_ref`: [kernel/src/lib.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/lib.rs)).

### Governing-rule requirement

No PolicyRule introduces this spec in-session yet; out of scope for this waypoint.

## Scope

In scope: GPU buffer/bind-group resources and the double-buffer swap
contract. Out of scope: what gets drawn with those buffers (see
[render](../render)) and how dirty ranges are computed (see
[svo](../svo)).