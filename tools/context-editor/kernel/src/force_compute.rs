//! Force compute shader & SVO collision — applies forces to particles and
//! resolves collisions against the Sparse Voxel Octree.
//!
//! ## Force Types
//!
//! | Type | Effect |
//! |------|--------|
//! | Explosion (0) | Radial push from origin |
//! | Attraction (1) | Pull towards origin |
//! | Vortex (2) | Tangential spin around origin |
//!
//! ## SVO Collision
//!
//! Particles ray-march their motion vector against the SVO to detect
//! solid voxel hits, then reflect with restitution and friction.

use bevy::prelude::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bytemuck::{Pod, Zeroable};

/// Maximum number of concurrent force events.
pub const MAX_FORCE_EVENTS: usize = 16;

// ---------------------------------------------------------------------------
// Force event types
// ---------------------------------------------------------------------------

/// Discrete force type tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ForceType {
    Explosion = 0,
    Attraction = 1,
    Vortex = 2,
}

/// CPU-side force event.
#[derive(Clone, Debug)]
pub struct ForceEvent {
    pub origin: Vec3,
    pub radius: f32,
    pub strength: f32,
    pub force_type: ForceType,
}

/// GPU-packed force event (32 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ForceEventGpu {
    pub origin: [f32; 3],
    pub radius: f32,
    pub strength: f32,
    pub force_type: u32,
    pub _pad: [f32; 2],
}

/// Uniform for the force compute dispatch.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ForceUniforms {
    pub delta_time: f32,
    pub force_count: u32,
    pub restitution: f32,
    pub friction: f32,
}

// ---------------------------------------------------------------------------
// ECS Resources
// ---------------------------------------------------------------------------

/// Queue of force events to apply this frame.
#[derive(Resource, Default)]
pub struct ForceEventQueue {
    pub events: Vec<ForceEvent>,
}

/// GPU buffer holding the packed force events.
#[derive(Resource)]
pub struct ForceBuffer {
    pub event_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
    pub count: u32,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ForceComputePlugin;

impl Plugin for ForceComputePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ForceEventQueue>();
        app.add_systems(
            PostUpdate,
            (init_force_resources, update_force_buffers).chain(),
        );
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn init_force_resources(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<ForceBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };
    let dev = device.wgpu_device();

    let event_buffer = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("force_event_buffer"),
        size: (MAX_FORCE_EVENTS * std::mem::size_of::<ForceEventGpu>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let uniform_buffer = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("force_uniforms"),
        size: std::mem::size_of::<ForceUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    commands.insert_resource(ForceBuffer {
        event_buffer,
        uniform_buffer,
        count: 0,
    });
}

fn update_force_buffers(
    mut queue_res: ResMut<ForceEventQueue>,
    render_queue: Option<Res<RenderQueue>>,
    time: Res<Time>,
    mut force_buf: Option<ResMut<ForceBuffer>>,
) {
    let (Some(rq), Some(ref mut buf)) = (render_queue, force_buf.as_mut()) else {
        return;
    };

    let count = queue_res.events.len().min(MAX_FORCE_EVENTS);
    buf.count = count as u32;

    // Write events
    if count > 0 {
        let gpu_events: Vec<ForceEventGpu> = queue_res
            .events
            .iter()
            .take(count)
            .map(|e| ForceEventGpu {
                origin: e.origin.to_array(),
                radius: e.radius,
                strength: e.strength,
                force_type: e.force_type as u32,
                _pad: [0.0; 2],
            })
            .collect();

        rq.write_buffer(&buf.event_buffer, 0, bytemuck::cast_slice(&gpu_events));
    }

    // Write uniforms
    let uniforms = ForceUniforms {
        delta_time: time.delta_secs(),
        force_count: count as u32,
        restitution: 0.4,
        friction: 0.8,
    };
    rq.write_buffer(&buf.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

    // Clear events for next frame
    queue_res.events.clear();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_event_gpu_is_32_bytes() {
        assert_eq!(std::mem::size_of::<ForceEventGpu>(), 32);
    }

    #[test]
    fn force_uniforms_is_16_bytes() {
        assert_eq!(std::mem::size_of::<ForceUniforms>(), 16);
    }

    #[test]
    fn force_type_tags() {
        assert_eq!(ForceType::Explosion as u32, 0);
        assert_eq!(ForceType::Attraction as u32, 1);
        assert_eq!(ForceType::Vortex as u32, 2);
    }

    #[test]
    fn pack_force_event() {
        let event = ForceEvent {
            origin: Vec3::new(1.0, 2.0, 3.0),
            radius: 10.0,
            strength: 50.0,
            force_type: ForceType::Explosion,
        };
        let gpu = ForceEventGpu {
            origin: event.origin.to_array(),
            radius: event.radius,
            strength: event.strength,
            force_type: event.force_type as u32,
            _pad: [0.0; 2],
        };
        assert_eq!(gpu.origin, [1.0, 2.0, 3.0]);
        assert_eq!(gpu.force_type, 0);
    }
}
