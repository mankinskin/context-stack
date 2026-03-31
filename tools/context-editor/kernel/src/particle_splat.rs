//! Motion-blurred particle splatting — injects dynamic particles into the
//! voxel splat pipeline with velocity-based AABB stretch for motion blur.
//!
//! Particles are stored as `ParticleSplat` GPU structs and appended to the
//! main splat buffer alongside SVO-derived voxel splats. The radix sort and
//! tiled rasteriser process them identically.

use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bytemuck::{Pod, Zeroable};

/// Maximum number of dynamic particles in the GPU buffer.
pub const MAX_PARTICLES: usize = 100_000;

// ---------------------------------------------------------------------------
// GPU Structs
// ---------------------------------------------------------------------------

/// GPU-side particle with position, velocity, colour, and scale.
///
/// 48 bytes (12 × f32). Velocity is used for:
/// - Motion blur (AABB stretch in sort-key build)
/// - Force compute integration
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct ParticleSplat {
    pub position: [f32; 3],
    pub scale: f32,
    pub velocity: [f32; 3],
    pub opacity: f32,
    pub color: [f32; 4],
}

/// Per-frame uniform for particle rendering.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ParticleUniforms {
    pub particle_count: u32,
    pub motion_blur_scale: f32,
    pub _pad: [f32; 2],
}

// ---------------------------------------------------------------------------
// ECS Resources
// ---------------------------------------------------------------------------

/// Emitter description for spawning particles.
#[derive(Clone, Debug)]
pub struct ParticleEmitter {
    pub origin: Vec3,
    pub rate: u32,
    pub color: [f32; 4],
    pub scale: f32,
    pub initial_velocity: Vec3,
    pub lifetime: f32,
}

/// CPU-side particle instance (with lifetime tracking).
#[derive(Clone, Debug)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub color: [f32; 4],
    pub scale: f32,
    pub opacity: f32,
    pub lifetime: f32,
    pub age: f32,
}

/// Resource holding live particles and emitters.
#[derive(Resource, Default)]
pub struct ParticleSystem {
    pub emitters: Vec<ParticleEmitter>,
    pub particles: Vec<Particle>,
}

/// GPU buffer for particle splats.
#[derive(Resource, Clone, ExtractResource)]
pub struct ParticleSplatBuffer {
    pub buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
    pub count: u32,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ParticleSplatPlugin;

impl Plugin for ParticleSplatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticleSystem>();
        app.add_systems(Update, simulate_particles);
        app.add_systems(
            PostUpdate,
            (init_particle_buffers, upload_particles).chain(),
        );
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Simulate particles: emit new, age existing, remove dead.
fn simulate_particles(time: Res<Time>, mut system: ResMut<ParticleSystem>) {
    let dt = time.delta_secs();

    // Diagnostic: log particle count at milestones so the browser console
    // confirms the simulation is running and emitters are seeded.
    let count_before = system.particles.len();

    // Emit new particles
    let mut new_particles = Vec::new();
    for emitter in &system.emitters {
        for _ in 0..emitter.rate {
            if system.particles.len() + new_particles.len() >= MAX_PARTICLES {
                break;
            }
            // Golden-angle sunflower jitter so each spawn goes to a slightly
            // different XZ position — prevents all particles stacking at
            // exactly the origin and creates a natural-looking cloud spread.
            let spawn_idx = (system.particles.len() + new_particles.len()) as f32;
            let angle = spawn_idx * 2.399_963; // golden angle ≈ 137.5°
            let radius = spawn_idx.sqrt() * 0.25; // grows slowly, ~3 units at 144 particles
            let jitter = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
            new_particles.push(Particle {
                position: emitter.origin + jitter,
                velocity: emitter.initial_velocity,
                color: emitter.color,
                scale: emitter.scale,
                opacity: 1.0,
                lifetime: emitter.lifetime,
                age: 0.0,
            });
        }
    }
    system.particles.extend(new_particles);

    #[cfg(target_arch = "wasm32")]
    {
        let new_total = system.particles.len();
        if count_before == 0 && new_total > 0 {
            web_sys::console::log_1(
                &format!("[particles] first {} particles spawned, {} emitters",
                    new_total, system.emitters.len()).into()
            );
        }
    }

    // Age and integrate
    for p in &mut system.particles {
        p.age += dt;
        p.position += p.velocity * dt;
        // Fade out over lifetime
        p.opacity = (1.0 - p.age / p.lifetime).max(0.0);
    }

    // Remove dead particles
    system.particles.retain(|p| p.age < p.lifetime);
}

fn init_particle_buffers(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<ParticleSplatBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };
    let dev = device.wgpu_device();

    let buffer = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("particle_splat_buffer"),
        size: (MAX_PARTICLES * std::mem::size_of::<ParticleSplat>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let uniform_buffer = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("particle_uniforms"),
        size: std::mem::size_of::<ParticleUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    commands.insert_resource(ParticleSplatBuffer {
        buffer,
        uniform_buffer,
        count: 0,
    });
}

fn upload_particles(
    system: Res<ParticleSystem>,
    queue: Option<Res<RenderQueue>>,
    mut buf: Option<ResMut<ParticleSplatBuffer>>,
) {
    let (Some(rq), Some(ref mut buf)) = (queue, buf.as_mut()) else {
        return;
    };

    let count = system.particles.len().min(MAX_PARTICLES);
    buf.count = count as u32;

    if count > 0 {
        let gpu_particles: Vec<ParticleSplat> = system
            .particles
            .iter()
            .take(count)
            .map(|p| ParticleSplat {
                position: p.position.to_array(),
                scale: p.scale,
                velocity: p.velocity.to_array(),
                opacity: p.opacity,
                color: p.color,
            })
            .collect();

        rq.write_buffer(&buf.buffer, 0, bytemuck::cast_slice(&gpu_particles));
    }

    let uniforms = ParticleUniforms {
        particle_count: count as u32,
        motion_blur_scale: 0.01,
        _pad: [0.0; 2],
    };
    rq.write_buffer(&buf.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_splat_is_48_bytes() {
        assert_eq!(std::mem::size_of::<ParticleSplat>(), 48);
    }

    #[test]
    fn particle_uniforms_is_16_bytes() {
        assert_eq!(std::mem::size_of::<ParticleUniforms>(), 16);
    }

    #[test]
    fn particle_fade_out() {
        let mut p = Particle {
            position: Vec3::ZERO,
            velocity: Vec3::X,
            color: [1.0; 4],
            scale: 0.1,
            opacity: 1.0,
            lifetime: 2.0,
            age: 0.0,
        };
        // Simulate half lifetime
        p.age = 1.0;
        p.opacity = (1.0 - p.age / p.lifetime).max(0.0);
        assert!((p.opacity - 0.5).abs() < 0.001);

        // At end of lifetime
        p.age = 2.0;
        p.opacity = (1.0 - p.age / p.lifetime).max(0.0);
        assert_eq!(p.opacity, 0.0);
    }

    #[test]
    fn max_particles_limit() {
        assert_eq!(MAX_PARTICLES, 100_000);
    }
}
