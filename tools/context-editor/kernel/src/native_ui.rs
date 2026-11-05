use std::time::Instant;

use anyrender::ImageRenderer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::RenderLayers,
    input::{
        keyboard::{
            Key as BevyKey,
            KeyboardInput,
        },
        mouse::{
            MouseButtonInput,
            MouseScrollUnit,
            MouseWheel,
        },
        ButtonState,
        InputSystems,
    },
    log::{
        debug,
        info,
    },
    prelude::*,
    render::render_resource::{
        Extent3d,
        TextureDimension,
        TextureFormat,
    },
    window::{
        Ime,
        PrimaryWindow,
        WindowFocused,
    },
};
use blitz_dom::{
    Document,
    DocumentConfig,
};
use blitz_traits::{
    events::{
        BlitzImeEvent,
        BlitzKeyEvent,
        BlitzMouseButtonEvent,
        KeyState,
        MouseEventButton,
        MouseEventButtons,
        UiEvent,
    },
    shell::{
        ColorScheme,
        Viewport,
    },
};
use dioxus::prelude::VirtualDom;
use dioxus_native_dom::DioxusDocument;
use keyboard_types::{
    Code,
    Key,
    Location,
    Modifiers,
};

const UI_LAYER: usize = 31;

pub struct NativeUiPlugin;

impl Plugin for NativeUiPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.add_systems(Startup, setup);
        app.add_systems(
            PreUpdate,
            handle_input
                .after(InputSystems)
                .run_if(resource_exists::<UiTexture>),
        );
        app.add_systems(PostUpdate, paint.run_if(resource_exists::<UiTexture>));
    }
}

struct NativeUi {
    document: DioxusDocument,
    painter: VelloCpuImageRenderer,
    size: UVec2,
    scale: f32,
    dirty: bool,
    buttons: MouseEventButtons,
    cursor: Option<Vec2>,
    mouse_captured: bool,
    keyboard: ButtonInput<KeyCode>,
    epoch: Instant,
}

#[derive(Resource)]
struct UiTexture(Handle<Image>);

#[derive(Component)]
struct UiOverlay;

fn setup(world: &mut World) -> Result {
    let window = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .single(world)?
        .clone();
    let size = viewport_size(&window)?;
    let mut document = DioxusDocument::new(
        VirtualDom::new(crate::ui::root_app),
        DocumentConfig {
            viewport: Some(viewport(&window)),
            ..default()
        },
    );
    document.add_user_agent_stylesheet(include_str!("native_ui.css"));
    document.initial_build();
    document.resolve(0.0);
    let image = world
        .resource_mut::<Assets<Image>>()
        .add(transparent_image(size));
    world.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderLayers::layer(UI_LAYER),
        Msaa::Off,
    ));
    world.spawn((
        Sprite {
            image: image.clone(),
            custom_size: Some(Vec2::new(window.width(), window.height())),
            ..default()
        },
        RenderLayers::layer(UI_LAYER),
        UiOverlay,
    ));
    world.insert_resource(UiTexture(image));
    world.insert_non_send_resource(NativeUi {
        document,
        painter: VelloCpuImageRenderer::new(size.x, size.y),
        size,
        scale: window.scale_factor(),
        dirty: true,
        buttons: MouseEventButtons::None,
        cursor: None,
        mouse_captured: false,
        keyboard: ButtonInput::default(),
        epoch: Instant::now(),
    });
    info!("Native Dioxus UI initialized in the Bevy window");
    Ok(())
}

fn viewport_size(window: &Window) -> Result<UVec2> {
    let size = UVec2::new(
        window.physical_width().max(1),
        window.physical_height().max(1),
    );
    if size.max_element() > 8192 {
        return Err(
            "native UI supports window dimensions up to 8192 pixels".into()
        );
    }
    Ok(size)
}

fn viewport(window: &Window) -> Viewport {
    Viewport::new(
        window.physical_width().max(1),
        window.physical_height().max(1),
        window.scale_factor(),
        ColorScheme::Dark,
    )
}

fn transparent_image(size: UVec2) -> Image {
    Image::new_fill(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0; 4],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn in_panel(
    document: &DioxusDocument,
    mut id: usize,
) -> bool {
    while let Some(node) = document.get_node(id) {
        if node
            .attr(blitz_dom::local_name!("class"))
            .is_some_and(|classes| {
                classes.split_whitespace().any(|c| c == "glass-panel")
            })
        {
            return true;
        }
        let Some(parent) = node.parent else { break };
        id = parent;
    }
    false
}

fn modifiers(keys: &ButtonInput<KeyCode>) -> Modifiers {
    let mut modifiers = Modifiers::empty();
    for (left, right, flag) in [
        (KeyCode::ShiftLeft, KeyCode::ShiftRight, Modifiers::SHIFT),
        (
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            Modifiers::CONTROL,
        ),
        (KeyCode::AltLeft, KeyCode::AltRight, Modifiers::ALT),
        (KeyCode::SuperLeft, KeyCode::SuperRight, Modifiers::META),
    ] {
        if keys.pressed(left) || keys.pressed(right) {
            modifiers.insert(flag);
        }
    }
    modifiers
}

fn logical_key(key: &BevyKey) -> Key {
    match key {
        BevyKey::Character(text) => Key::Character(text.to_string()),
        BevyKey::Space => Key::Character(" ".into()),
        BevyKey::Unidentified(_) => Key::Unidentified,
        key => match format!("{key:?}").parse() {
            Ok(key) => key,
            Err(error) => {
                debug!("Unsupported native UI key {key:?}: {error}");
                Key::Unidentified
            },
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_input(
    mut ui: NonSendMut<NativeUi>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut mouse_events: MessageReader<MouseButtonInput>,
    mut keyboard_events: MessageReader<KeyboardInput>,
    mut wheel_events: MessageReader<MouseWheel>,
    mut ime_events: MessageReader<Ime>,
    mut focus_events: MessageReader<WindowFocused>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
) -> Result {
    for event in focus_events.read() {
        if !event.focused {
            ui.document.clear_focus();
            ui.buttons = MouseEventButtons::None;
            ui.mouse_captured = false;
            ui.keyboard.reset_all();
            ui.dirty = true;
        }
    }
    let mods = modifiers(&ui.keyboard);
    let position = window.cursor_position();
    let hit = position.and_then(|p| ui.document.hit(p.x, p.y));
    let over_panel = hit.is_some_and(|hit| in_panel(&ui.document, hit.node_id));
    if let Some(position) = position {
        let event = BlitzMouseButtonEvent {
            x: position.x,
            y: position.y,
            button: MouseEventButton::Main,
            buttons: ui.buttons,
            mods,
        };
        if ui.cursor != Some(position) {
            ui.document
                .handle_ui_event(UiEvent::MouseMove(event.clone()));
            ui.cursor = Some(position);
            ui.dirty = true;
        }
        for input in mouse_events.read() {
            let button = match input.button {
                MouseButton::Left => MouseEventButton::Main,
                MouseButton::Right => MouseEventButton::Secondary,
                MouseButton::Middle => MouseEventButton::Auxiliary,
                MouseButton::Back => MouseEventButton::Fourth,
                MouseButton::Forward => MouseEventButton::Fifth,
                MouseButton::Other(_) => continue,
            };
            let mask = MouseEventButtons::from(button);
            let pressed = input.state == ButtonState::Pressed;
            if pressed && !over_panel && !ui.mouse_captured {
                ui.document.clear_focus();
                ui.dirty = true;
                continue;
            }
            if pressed && over_panel {
                ui.mouse_captured = true;
            }
            if !ui.mouse_captured {
                continue;
            }
            ui.buttons.set(mask, pressed);
            let event = BlitzMouseButtonEvent {
                button,
                buttons: ui.buttons,
                ..event.clone()
            };
            ui.document.handle_ui_event(if pressed {
                UiEvent::MouseDown(event)
            } else {
                UiEvent::MouseUp(event)
            });
            ui.dirty = true;
        }
    } else {
        mouse_events.clear();
        ui.cursor = None;
        if !mouse.any_pressed([
            MouseButton::Left,
            MouseButton::Middle,
            MouseButton::Right,
        ]) {
            ui.buttons = MouseEventButtons::None;
            ui.mouse_captured = false;
        }
    }
    for wheel in wheel_events.read() {
        if let Some(hit) = hit.filter(|_| over_panel) {
            let factor = if wheel.unit == MouseScrollUnit::Line {
                32.0
            } else {
                1.0
            };
            ui.document.scroll_node_by(
                hit.node_id,
                (wheel.x * factor) as f64,
                (wheel.y * factor) as f64,
            );
            ui.dirty = true;
        }
    }
    for input in keyboard_events.read() {
        match input.state {
            ButtonState::Pressed => ui.keyboard.press(input.key_code),
            ButtonState::Released => ui.keyboard.release(input.key_code),
        }
        let ui_focused = ui
            .document
            .get_focussed_node_id()
            .is_some_and(|id| in_panel(&ui.document, id));
        if !ui_focused && input.key_code != KeyCode::Tab {
            continue;
        }
        let code = match format!("{:?}", input.key_code).parse::<Code>() {
            Ok(code) => code,
            Err(error) => {
                debug!(
                    "Unsupported native UI key code {:?}: {error}",
                    input.key_code
                );
                Code::Unidentified
            },
        };
        let event = BlitzKeyEvent {
            key: logical_key(&input.logical_key),
            code,
            modifiers: modifiers(&ui.keyboard),
            location: Location::Standard,
            is_auto_repeating: input.repeat,
            is_composing: false,
            state: if input.state == ButtonState::Pressed {
                KeyState::Pressed
            } else {
                KeyState::Released
            },
            text: input.text.as_ref().map(|text| text.as_str().into()),
        };
        ui.document
            .handle_ui_event(if input.state == ButtonState::Pressed {
                UiEvent::KeyDown(event)
            } else {
                UiEvent::KeyUp(event)
            });
        ui.dirty = true;
    }
    let ui_focused = ui
        .document
        .get_focussed_node_id()
        .is_some_and(|id| in_panel(&ui.document, id));
    for event in ime_events.read() {
        if !ui_focused {
            continue;
        }
        let event = match event {
            Ime::Enabled { .. } => BlitzImeEvent::Enabled,
            Ime::Disabled { .. } => BlitzImeEvent::Disabled,
            Ime::Preedit { value, cursor, .. } =>
                BlitzImeEvent::Preedit(value.clone(), *cursor),
            Ime::Commit { value, .. } => BlitzImeEvent::Commit(value.clone()),
        };
        ui.document.handle_ui_event(UiEvent::Ime(event));
        ui.dirty = true;
    }
    if over_panel || ui.mouse_captured {
        mouse.reset_all();
    }
    if ui.buttons == MouseEventButtons::None {
        ui.mouse_captured = false;
    }
    if ui_focused {
        keys.reset_all();
    }
    Ok(())
}

fn paint(
    mut ui: NonSendMut<NativeUi>,
    texture: Res<UiTexture>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut sprites: Query<&mut Sprite, With<UiOverlay>>,
) -> Result {
    let size = viewport_size(&window)?;
    if size != ui.size || window.scale_factor() != ui.scale {
        ui.document.set_viewport(viewport(&window));
        ui.painter.resize(size.x, size.y);
        ui.size = size;
        ui.scale = window.scale_factor();
        *images
            .get_mut(&texture.0)
            .ok_or("native UI image is missing")? = transparent_image(size);
        sprites.single_mut()?.custom_size =
            Some(Vec2::new(window.width(), window.height()));
        ui.dirty = true;
    }
    let changed = ui.document.poll(None);
    if !ui.dirty && !changed {
        return Ok(());
    }
    let elapsed = ui.epoch.elapsed().as_secs_f64();
    ui.document.resolve(elapsed);
    let NativeUi {
        document,
        painter,
        scale,
        ..
    } = &mut *ui;
    painter.reset();
    let mut pixels = Vec::new();
    painter.render_to_vec(
        |scene| {
            blitz_paint::paint_scene(
                scene,
                document,
                f64::from(*scale),
                size.x,
                size.y,
            )
        },
        &mut pixels,
    );
    // Vello produces premultiplied RGBA; Bevy's sprite material expects straight alpha.
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        if alpha > 0 && alpha < 255 {
            for channel in &mut pixel[..3] {
                *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha)
                    .min(255) as u8;
            }
        }
    }
    images
        .get_mut(&texture.0)
        .ok_or("native UI image is missing")?
        .data = Some(pixels);
    ui.dirty = false;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::prelude::*;
    use std::sync::atomic::{
        AtomicUsize,
        Ordering,
    };

    static CLICKS: AtomicUsize = AtomicUsize::new(0);
    static INPUT_LENGTH: AtomicUsize = AtomicUsize::new(0);

    fn fixture() -> Element {
        let mut text = use_signal(String::new);
        rsx! {
            div { class: "glass-panel", style: "width: 200px; height: 200px",
                button {
                    id: "action",
                    onclick: move |_| { CLICKS.fetch_add(1, Ordering::SeqCst); },
                    "Action"
                }
                input {
                    id: "entry",
                    value: "{text}",
                    oninput: move |event| {
                        INPUT_LENGTH.store(event.value().len(), Ordering::SeqCst);
                        text.set(event.value());
                    }
                }
            }
        }
    }

    fn click(
        document: &mut DioxusDocument,
        selector: &str,
    ) {
        let mut painter = VelloCpuImageRenderer::new(400, 300);
        painter.render_to_vec(
            |scene| blitz_paint::paint_scene(scene, document, 1.0, 400, 300),
            &mut Vec::new(),
        );
        let id = document.query_selector(selector).unwrap().unwrap();
        let node = document.get_node(id).unwrap();
        let position = node.absolute_position(0.0, 0.0);
        let event = BlitzMouseButtonEvent {
            x: position.x + node.final_layout.size.width / 2.0,
            y: position.y + node.final_layout.size.height / 2.0,
            button: MouseEventButton::Main,
            buttons: MouseEventButtons::from(MouseEventButton::Main),
            mods: Modifiers::empty(),
        };
        assert!(in_panel(document, id));
        document.handle_ui_event(UiEvent::MouseMove(event.clone()));
        document.handle_ui_event(UiEvent::MouseDown(event.clone()));
        document.handle_ui_event(UiEvent::MouseUp(BlitzMouseButtonEvent {
            buttons: MouseEventButtons::None,
            ..event
        }));
        document.poll(None);
        document.resolve(0.0);
    }

    #[test]
    fn native_document_renders_and_dispatches_clicks_and_text() {
        CLICKS.store(0, Ordering::SeqCst);
        INPUT_LENGTH.store(0, Ordering::SeqCst);
        let mut document = DioxusDocument::new(
            VirtualDom::new(fixture),
            DocumentConfig {
                viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Dark)),
                ..default()
            },
        );
        document.add_user_agent_stylesheet(include_str!("native_ui.css"));
        document.initial_build();
        document.resolve(0.0);
        click(&mut document, "#action");
        assert_eq!(CLICKS.load(Ordering::SeqCst), 1);
        click(&mut document, "#entry");
        for ch in ['h', 'i'] {
            document.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
                key: Key::Character(ch.to_string()),
                code: Code::Unidentified,
                modifiers: Modifiers::empty(),
                location: Location::Standard,
                is_auto_repeating: false,
                is_composing: false,
                state: KeyState::Pressed,
                text: Some(ch.to_string().into()),
            }));
        }
        document.poll(None);
        document.resolve(0.0);
        assert_eq!(INPUT_LENGTH.load(Ordering::SeqCst), 2);

        let mut painter = VelloCpuImageRenderer::new(400, 300);
        let mut pixels = Vec::new();
        painter.render_to_vec(
            |scene| blitz_paint::paint_scene(scene, &document, 1.0, 400, 300),
            &mut pixels,
        );
        assert_eq!(pixels.len(), 400 * 300 * 4);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
        assert_eq!(pixels[(299 * 400 + 399) * 4 + 3], 0);

        document.set_viewport(Viewport::new(600, 400, 2.0, ColorScheme::Dark));
        document.resolve(1.0);
        painter.resize(600, 400);
        painter.render_to_vec(
            |scene| blitz_paint::paint_scene(scene, &document, 2.0, 600, 400),
            &mut pixels,
        );
        assert_eq!(pixels.len(), 600 * 400 * 4);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }

    #[test]
    fn maps_native_text_and_navigation_keys() {
        assert_eq!(logical_key(&BevyKey::Space), Key::Character(" ".into()));
        assert_eq!(logical_key(&BevyKey::Enter), Key::Enter);
        assert_eq!(logical_key(&BevyKey::Backspace), Key::Backspace);
        assert_eq!(
            logical_key(&BevyKey::Character("a".into())),
            Key::Character("a".into())
        );
    }
}
