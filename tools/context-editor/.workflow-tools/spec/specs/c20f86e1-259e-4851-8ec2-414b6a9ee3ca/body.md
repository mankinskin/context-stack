&lt;!-- aligned-structure:v2 --&gt;

# context-editor/sandbox/ui-bridge

## Target Code Location

- [kernel/src/ui/mod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/mod.rs) — `root_app` (the Dioxus root component wired into `dioxus::launch` from `lib.rs::launch`), `GlassPanel`.
- [kernel/src/ui/bridge.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/bridge.rs) — `UiBridgePlugin`.
- [kernel/src/ui/interaction.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/interaction.rs) — `InteractionBridgePlugin`.
- [kernel/src/ui/inventory.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/inventory.rs), [kernel/src/ui/panel_interaction.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/panel_interaction.rs), [kernel/src/ui/world_panel.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/world_panel.rs), [kernel/src/ui/code_viewer.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/code_viewer.rs), [kernel/src/ui/doc_editor.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/doc_editor.rs), [kernel/src/ui/skill.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/skill.rs), [kernel/src/ui/ticket_editor.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/ticket_editor.rs) — the remaining 7 of 9 Dioxus UI submodules.
- [kernel/src/lib.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/lib.rs) — `launch<W: SandboxWorld + Default>()`, the single public entry point this module's `root_app` is wired into.
- [sandbox-app/src/world.rs](../../../../context-engine/context-stack/tools/context-editor/sandbox-app/src/world.rs) — `SandboxWorld` app-side implementor; sidebar/inventory `rsx!` stubs consumed by this contract.

## Naming Conventions

- `root_app` — the Dioxus root component; calls `crate::world().sidebar_content()` / `.inventory_content()`.
- `world()` — returns the global `Arc<dyn SandboxWorld>`, set exactly once by `lib.rs::launch()`.
- `UiBridgePlugin`, `InteractionBridgePlugin`, `InventoryPlugin`, `PanelInteractionPlugin`, `WorldPanelPlugin`, `SkillPlugin`, `TicketEditorPlugin` — the 9 crate-root re-exports use a **renaming re-export**: `ui::{bridge as ui_bridge, code_viewer, doc_editor, interaction, inventory, panel_interaction, skill, ticket_editor, world_panel}`, disambiguating `bridge` from other crate-root names.
- Criterion-id scheme: `context-editor-sandbox-ui-bridge-<topic>` (e.g. `context-editor-sandbox-ui-bridge-sandbox-world-call`).

## Requester Input

> Camera resolution controls are missing. Several sliders are not yet reliable. For example, when Wireframe Grid is enabled, it may not appear or may not be sized correctly.

## Reading Order

1. [context-editor/sandbox](../sandbox) — parent root, Shared Invariant 4 (`launch()` sets `SandboxWorld` exactly once) governs this child directly.
2. [context-editor/sandbox/editor](../editor) — provider of `crate::editor::debug_overlay::DebugPanel`, imported directly by `ui/mod.rs`.
3. [ticket 5162dae2 Interaction Bridge: 2D Unprojection & Dioxus-to-WASM Pipeline](../../../../.workflow-tools/ticket/tickets/5162dae2-8222-49af-a2e0-f508541ef678/ticket.toml) — done ticket this module's `InteractionBridgePlugin`/`UiBridgePlugin` reuse (W1 §5 "Reuse, with an inherited evidence gap").
4. [dioxus SKILL.md](../../../../workflow-tools/.agents/skills/dioxus/SKILL.md) — governing Dioxus authoring guidance for this bridge's component/signal patterns.

## Responsibility

`ui-bridge` owns the Dioxus-to-kernel integration point: the Dioxus root
component (`root_app`), its 9 submodule plugins (bridge, interaction,
inventory, panel interaction, world panel, code viewer, doc editor, skill,
ticket editor), and the one-directional call from Dioxus rendering into the
`SandboxWorld` trait object. It is "the one place the Dioxus atomics/events
surface meets the kernel's `SandboxWorld` trait object," per W1, and is not
extractable without redesigning `launch()`.

## Interfaces And Dependencies

- Depends on [editor](../editor) (`crate::editor::debug_overlay::DebugPanel`, imported directly in `ui/mod.rs`).
- Depends on the app-side `SandboxWorld` implementor (`sandbox-app/src/world.rs`, outside this hierarchy) via the global `world()` accessor set once by `lib.rs::launch()`.
- Consumed by `build_kernel_app` and `lib.rs::launch()` directly (`dioxus::launch(ui::root_app)`).

## Behavior

`root_app` calls `crate::world().sidebar_content()` / `.inventory_content()` —
a static, one-directional call from Dioxus rendering into the `SandboxWorld`
trait object, confirmed by W1. `ui::interaction::InteractionBridgePlugin` and
`ui::bridge::UiBridgePlugin` are the Bevy-side plugins presumed to carry the
reverse direction (Dioxus event → Bevy system), but W1 explicitly did not
trace their internals to confirm the exact queue/channel mechanism — this is
an inherited evidence gap (also noted against [ticket 5162dae2](../../../../.workflow-tools/ticket/tickets/5162dae2-8222-49af-a2e0-f508541ef678/ticket.toml)), not resolved by this spec.

## Boundaries And Failure Cases

- `lib.rs::launch()` panics on a second call (per its own doc comment, confirmed by W1) — this module's `root_app` and the global `world()` static must never be initialized twice in one process.
- The reverse Dioxus-event → Bevy-system direction through `InteractionBridgePlugin`/`UiBridgePlugin` is an unconfirmed mechanism (see Behavior); a future defect-fix session investigating unreliable sliders (Requester Input above) must trace this path before assuming a specific failure point.
- Sandbox-app sidebar and inventory `rsx!` content are placeholder stubs (`sandbox-app/src/world.rs`), so `root_app`'s calls to `sidebar_content()`/`inventory_content()` currently render near-empty panels — not a bug in this module, but a boundary this spec's dependents must not mistake for a defect in the bridge itself.

## Provider/Consumer Contract

- **Consumes from** [editor](../editor): `DebugPanel` for the Dioxus debug overlay.
- **Consumes from** app-side `SandboxWorld` (outside this hierarchy): `sidebar_content()`/`inventory_content()` trait methods.
- **Provides to** `lib.rs::launch()`: the `root_app` Dioxus root component.

## Examples

A defect-fix session (W6) investigating "several sliders are not yet reliable" reads this child's Behavior/Boundaries sections, learns the Dioxus→Bevy reverse-direction mechanism through `InteractionBridgePlugin` is an unconfirmed internal, and knows to trace that plugin's internals as a first step rather than assuming the slider UI component itself is at fault.

## Evidence

### Guards

- [`context-editor-kernel-native-tests`](../../../../.workflow-tools/test/specs/context-editor-kernel-native-tests.json) — `cargo test -p context-editor-kernel`; exercises 9 UI test seams (second-largest count of any module per W1), included in the 389-pass baseline.
- [`context-editor-kernel-wasm-check`](../../../../.workflow-tools/test/specs/context-editor-kernel-wasm-check.json) — `cargo check --target wasm32-unknown-unknown -p context-editor-kernel`; `ui/bridge.rs` carries a `#[cfg(target_arch = "wasm32")]` gate not exercised by the native run. Execution pending.
- [`context-editor-sandbox-app-trunk-build`](../../../../.workflow-tools/test/specs/context-editor-sandbox-app-trunk-build.json) — `trunk build`; the only guard that exercises `root_app`'s `dioxus::launch` wiring end to end. Execution pending as of this spec's creation.

### Positions

- **implemented** — `root_app`, the global `world()` accessor, 9 UI submodule plugins (`code_ref`: [kernel/src/ui/mod.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/mod.rs)).
- **partial** — the Dioxus-event → Bevy-system reverse direction through `InteractionBridgePlugin`/`UiBridgePlugin` is implemented but its exact mechanism is unconfirmed by this pass (`code_ref`: [kernel/src/ui/interaction.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/interaction.rs), [kernel/src/ui/bridge.rs](../../../../context-engine/context-stack/tools/context-editor/kernel/src/ui/bridge.rs)); camera-resolution controls and reliable sliders are user-reported as missing/unreliable and not yet fixed.
- **not-implemented** — sandbox-app sidebar/inventory content beyond placeholder stubs (`code_ref`: [sandbox-app/src/world.rs](../../../../context-engine/context-stack/tools/context-editor/sandbox-app/src/world.rs)).

### Governing-rule requirement

No PolicyRule introduces this spec in-session yet; out of scope for this waypoint.

## Scope

In scope: `root_app`, the global `SandboxWorld` accessor, and the 9 Dioxus UI
submodule plugins. Out of scope: the app-side `SandboxWorld` implementation
itself (`sandbox-app/src/world.rs`, a consumer, not part of this contract) and
brush-tool UX (see [editor](../editor)).