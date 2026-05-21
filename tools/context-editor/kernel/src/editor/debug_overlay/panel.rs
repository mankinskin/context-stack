use super::*;
use dioxus::prelude::*;
use std::time::Duration;

use crate::ui::{
    GlassPanel,
    TreeSection,
};

#[component]
pub fn DebugPanel() -> Element {
    let mut tick = use_signal(|| 0u32);
    let _ = *tick.read();

    use_future(move || async move {
        loop {
            futures_timer::Delay::new(Duration::from_millis(500)).await;
            tick.set(tick() + 1);
        }
    });

    let frame_us = FRAME_TIME_US.load(Ordering::Relaxed);
    let fps = if frame_us == 0 {
        0.0
    } else {
        1_000_000.0 / frame_us as f64
    };
    let ms = frame_us as f64 / 1000.0;
    let cpu_threads = CPU_THREADS.load(Ordering::Relaxed);
    let vram_mb = VRAM_ESTIMATE_MB.load(Ordering::Relaxed);

    let cam_x = CAMERA_X_DM.load(Ordering::Relaxed) as f32 / 10.0;
    let cam_y = CAMERA_Y_DM.load(Ordering::Relaxed) as f32 / 10.0;
    let cam_z = CAMERA_Z_DM.load(Ordering::Relaxed) as f32 / 10.0;
    let yaw_deg = CAMERA_YAW_DEG10.load(Ordering::Relaxed) as f32 / 10.0;
    let pitch_deg = CAMERA_PITCH_DEG10.load(Ordering::Relaxed) as f32 / 10.0;

    let node_count = SVO_NODE_COUNT.load(Ordering::Relaxed);
    let dirty = SVO_DIRTY_RANGES.load(Ordering::Relaxed);
    let max_depth = SVO_MAX_DEPTH.load(Ordering::Relaxed);
    let resolution = if max_depth > 0 { 1u64 << max_depth } else { 0 };
    let capacity = crate::gpu::SVO_CAPACITY_NODES as u64;

    let seed_hi = WORLD_SEED_HI.load(Ordering::Relaxed);
    let seed_lo = WORLD_SEED_LO.load(Ordering::Relaxed);
    let seed = ((seed_hi as u64) << 32) | (seed_lo as u64);
    let active_preset = WORLD_PRESET.load(Ordering::Relaxed);
    let preset_names = crate::world_preset_names();
    let active_preset_name = preset_names
        .get(active_preset as usize)
        .cloned()
        .unwrap_or_else(|| "?".to_string());

    let mut free_fly = use_signal(|| FREE_FLY_ENABLED.load(Ordering::Relaxed));
    let mut neighbor_blend =
        use_signal(|| NEIGHBOR_BLEND_ENABLED.load(Ordering::Relaxed));
    let mut shadow_rays =
        use_signal(|| SHADOW_RAYS_ENABLED.load(Ordering::Relaxed));
    let mut reflection_rays =
        use_signal(|| REFLECTION_RAYS_ENABLED.load(Ordering::Relaxed));
    let mut wireframe_en =
        use_signal(|| WIREFRAME_ENABLED.load(Ordering::Relaxed));
    let mut depth = use_signal(|| WIREFRAME_DEPTH.load(Ordering::Relaxed));
    let mut occupied =
        use_signal(|| WIREFRAME_OCCUPIED.load(Ordering::Relaxed));
    let mut lod_en = use_signal(|| LOD_ENABLED.load(Ordering::Relaxed));
    let mut lod_thresh =
        use_signal(|| LOD_THRESHOLD_X1000.load(Ordering::Relaxed));
    let mut lod_soft =
        use_signal(|| LOD_SOFTNESS_X1000.load(Ordering::Relaxed));

    rsx! {
        GlassPanel { title: "Debug".to_string(),
            div { class: "space-y-0.5 text-xs",
                TreeSection { label: "Performance".to_string(), default_open: true,
                    div { class: "flex justify-between text-white/70",
                        span { "FPS" }
                        span { class: "font-mono text-green-400", "{fps:.0}" }
                    }
                    div { class: "flex justify-between text-white/70",
                        span { "Frame" }
                        span { class: "font-mono", "{ms:.2} ms" }
                    }
                    div { class: "flex justify-between text-white/70",
                        span { "CPU threads" }
                        span { class: "font-mono", "{cpu_threads}" }
                    }
                    div { class: "flex justify-between text-white/70",
                        span { "VRAM est." }
                        span { class: "font-mono", "{vram_mb} MiB" }
                    }
                }

                TreeSection { label: "Camera".to_string(), default_open: true,
                    div { class: "grid grid-cols-2 gap-x-2 text-white/70",
                        span { "X" }
                        span { class: "font-mono text-right", "{cam_x:.1}" }
                        span { "Y" }
                        span { class: "font-mono text-right", "{cam_y:.1}" }
                        span { "Z" }
                        span { class: "font-mono text-right", "{cam_z:.1}" }
                        span { "Yaw" }
                        span { class: "font-mono text-right", "{yaw_deg:.1}°" }
                        span { "Pitch" }
                        span { class: "font-mono text-right", "{pitch_deg:.1}°" }
                    }
                    label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white mt-1.5",
                        input {
                            r#type: "checkbox",
                            checked: *free_fly.read(),
                            onclick: move |_| {
                                let v = !*free_fly.read();
                                free_fly.set(v);
                                FREE_FLY_ENABLED.store(v, Ordering::Relaxed);
                            }
                        }
                        "Free Fly  (Q/E = up/down)"
                    }
                }

                TreeSection { label: "SVO / Geometry".to_string(), default_open: false,
                    div { class: "grid grid-cols-2 gap-x-2 text-white/70",
                        span { "Max Depth" }
                        span { class: "font-mono text-right", "{max_depth}" }
                        span { "Resolution" }
                        span { class: "font-mono text-right", "{resolution}³" }
                        span { "Nodes" }
                        span { class: "font-mono text-right", "{node_count} / {capacity}" }
                        span { "Dirty ranges" }
                        span { class: "font-mono text-right text-yellow-400", "{dirty}" }
                    }
                    div { class: "flex justify-between text-white/70 mt-1.5",
                        span { "Seed" }
                        span { class: "font-mono text-[10px] text-white/50", "{seed:#018X}" }
                    }
                }

                TreeSection { label: "Rendering".to_string(), default_open: true,
                    TreeSection { label: "Ray March".to_string(), default_open: true,
                        label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                            input {
                                r#type: "checkbox",
                                checked: *neighbor_blend.read(),
                                onclick: move |_| {
                                    let v = !*neighbor_blend.read();
                                    neighbor_blend.set(v);
                                    NEIGHBOR_BLEND_ENABLED.store(v, Ordering::Relaxed);
                                }
                            }
                            "Neighbor Blend"
                        }
                        label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                            input {
                                r#type: "checkbox",
                                checked: *shadow_rays.read(),
                                onclick: move |_| {
                                    let v = !*shadow_rays.read();
                                    shadow_rays.set(v);
                                    SHADOW_RAYS_ENABLED.store(v, Ordering::Relaxed);
                                }
                            }
                            "Shadow Rays"
                        }
                        label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                            input {
                                r#type: "checkbox",
                                checked: *reflection_rays.read(),
                                onclick: move |_| {
                                    let v = !*reflection_rays.read();
                                    reflection_rays.set(v);
                                    REFLECTION_RAYS_ENABLED.store(v, Ordering::Relaxed);
                                }
                            }
                            "Reflections"
                        }
                        TreeSection { label: "LOD Cutoff".to_string(), default_open: true,
                            label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                                input {
                                    r#type: "checkbox",
                                    checked: *lod_en.read(),
                                    onclick: move |_| {
                                        let v = !*lod_en.read();
                                        lod_en.set(v);
                                        LOD_ENABLED.store(v, Ordering::Relaxed);
                                    }
                                }
                                "Enable LOD"
                            }
                            div { class: "text-white/70 mt-1",
                                div { class: "flex justify-between",
                                    span { "Threshold (px)" }
                                    span { class: "font-mono", "{*lod_thresh.read() as f32 / 1000.0:.1}" }
                                }
                                input {
                                    r#type: "range",
                                    min: "500",
                                    max: "64000",
                                    step: "500",
                                    value: "{lod_thresh}",
                                    class: "w-full accent-yellow-400",
                                    oninput: move |evt| {
                                        if let Ok(v) = evt.value().parse::<u32>() {
                                            lod_thresh.set(v);
                                            LOD_THRESHOLD_X1000.store(v, Ordering::Relaxed);
                                        }
                                    }
                                }
                            }
                            div { class: "text-white/70 mt-1",
                                div { class: "flex justify-between",
                                    span { "Softness (px)" }
                                    span { class: "font-mono", "{*lod_soft.read() as f32 / 1000.0:.1}" }
                                }
                                input {
                                    r#type: "range",
                                    min: "100",
                                    max: "8000",
                                    step: "100",
                                    value: "{lod_soft}",
                                    class: "w-full accent-yellow-400",
                                    oninput: move |evt| {
                                        if let Ok(v) = evt.value().parse::<u32>() {
                                            lod_soft.set(v);
                                            LOD_SOFTNESS_X1000.store(v, Ordering::Relaxed);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    TreeSection { label: "Wireframe".to_string(), default_open: false,
                        label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                            input {
                                r#type: "checkbox",
                                checked: *wireframe_en.read(),
                                onclick: move |_| {
                                    let v = !*wireframe_en.read();
                                    wireframe_en.set(v);
                                    WIREFRAME_ENABLED.store(v, Ordering::Relaxed);
                                }
                            }
                            "Enable  (F3)"
                        }
                        div { class: "text-white/70",
                            div { class: "flex justify-between",
                                span { "Depth  (F4 / F5)" }
                                span { class: "font-mono", "{depth}" }
                            }
                            input {
                                r#type: "range",
                                min: "0",
                                max: "11",
                                value: "{depth}",
                                class: "w-full accent-green-400",
                                oninput: move |evt| {
                                    if let Ok(v) = evt.value().parse::<u32>() {
                                        depth.set(v);
                                        WIREFRAME_DEPTH.store(v, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                        label { class: "flex items-center gap-2 cursor-pointer text-white/70 hover:text-white",
                            input {
                                r#type: "checkbox",
                                checked: *occupied.read(),
                                onclick: move |_| {
                                    let v = !*occupied.read();
                                    occupied.set(v);
                                    WIREFRAME_OCCUPIED.store(v, Ordering::Relaxed);
                                }
                            }
                            "Occupied Only  (F6)"
                        }
                    }
                }

                TreeSection { label: "World".to_string(), default_open: false,
                    div { class: "flex justify-between text-white/70 mb-1.5",
                        span { "Active" }
                        span { class: "font-mono text-cyan-400", "{active_preset_name}" }
                    }
                    div { class: "grid grid-cols-2 gap-1",
                        for (i, name) in preset_names.iter().enumerate() {
                            {
                                let idx = i as u32;
                                let name = name.clone();
                                let cls = if idx == active_preset {
                                    "px-2 py-0.5 text-[10px] rounded bg-cyan-500/40 border border-cyan-400 text-white"
                                } else {
                                    "px-2 py-0.5 text-[10px] rounded bg-white/10 hover:bg-white/20 border border-white/20 text-white/60"
                                };
                                rsx! {
                                    button {
                                        class: "{cls}",
                                        onclick: move |_| { WORLD_PRESET.store(idx, Ordering::Relaxed); },
                                        "{name}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
