use bevy::prelude::*;

/// Return the [`WindowPlugin`] configuration needed to attach Bevy to the
/// `#bevy-canvas` HTML element on WASM targets.
///
/// On native targets, returns the default `WindowPlugin` (windowed 1280x720).
///
/// Intended to be called inside the `sandbox-app` as:
/// ```rust,ignore
/// app.add_plugins(MinimalPlugins.set(context_editor_kernel::render::canvas_window_plugin()));
/// ```
pub fn canvas_window_plugin() -> WindowPlugin {
    WindowPlugin {
        primary_window: Some(Window {
            title: "Context Editor".into(),
            #[cfg(target_arch = "wasm32")]
            canvas: Some("#bevy-canvas".to_string()),
            #[cfg(target_arch = "wasm32")]
            fit_canvas_to_parent: true,
            #[cfg(target_arch = "wasm32")]
            prevent_default_event_handling: true,
            ..default()
        }),
        ..default()
    }
}

/// On WASM, force-sync the Bevy [`Window`] resolution from the actual canvas
/// element dimensions each frame. This works around Bevy's `fit_canvas_to_parent`
/// ResizeObserver not firing when the parent is already the correct size at
/// creation time.
#[cfg(target_arch = "wasm32")]
pub fn sync_canvas_resolution(mut windows: Query<&mut Window>) {
    let Some(canvas) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("bevy-canvas"))
    else {
        return;
    };
    let width = canvas.client_width().max(1) as f32;
    let height = canvas.client_height().max(1) as f32;

    for mut window in &mut windows {
        let current_width = window.resolution.physical_width() as f32;
        let current_height = window.resolution.physical_height() as f32;
        if (current_width - width).abs() > 1.0
            || (current_height - height).abs() > 1.0
        {
            window
                .resolution
                .set_physical_resolution(width as u32, height as u32);
        }
    }
}