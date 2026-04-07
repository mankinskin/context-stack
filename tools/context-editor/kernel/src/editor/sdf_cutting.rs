//! SDF Item Cutting — CSG shader subtraction, cut particles, and liquid glass
//! impact feedback.
//!
//! Provides runtime systems for:
//! - **CSG subtraction**: subtracts a tool SDF from the world SVO, producing a
//!   boolean-difference cut in the voxel grid.
//! - **Cut particles**: emits debris particles along the cut surface.
//! - **Liquid glass impact**: spawns glass-refraction splash feedback at the
//!   impact point.

use bevy::prelude::*;

// ---------------------------------------------------------------------------
// CSG operations
// ---------------------------------------------------------------------------

/// Supported CSG (Constructive Solid Geometry) boolean operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CsgOp {
    /// A ∪ B
    Union,
    /// A ∩ B
    Intersection,
    /// A − B (subtract B from A)
    Subtraction,
}

/// Evaluate a CSG boolean on two signed-distance values.
///
/// Standard SDF CSG definitions:
/// - `union(a,b) = min(a,b)`
/// - `intersection(a,b) = max(a,b)`
/// - `subtraction(a,b) = max(a, -b)`
pub fn sdf_csg(op: CsgOp, d_a: f32, d_b: f32) -> f32 {
    match op {
        CsgOp::Union => d_a.min(d_b),
        CsgOp::Intersection => d_a.max(d_b),
        CsgOp::Subtraction => d_a.max(-d_b),
    }
}

/// Smooth-minimum CSG union with blending radius `k`.
///
/// Produces a rounded transition between two SDFs.
pub fn sdf_smooth_union(d_a: f32, d_b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return d_a.min(d_b);
    }
    let h = (0.5 + 0.5 * (d_b - d_a) / k).clamp(0.0, 1.0);
    lerp(d_b, d_a, h) - k * h * (1.0 - h)
}

/// Smooth subtraction: smooth `max(a, -b)` with blending radius `k`.
pub fn sdf_smooth_subtraction(d_a: f32, d_b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return d_a.max(-d_b);
    }
    let h = (0.5 - 0.5 * (d_a + d_b) / k).clamp(0.0, 1.0);
    lerp(d_a, -d_b, h) + k * h * (1.0 - h)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ---------------------------------------------------------------------------
// SDF primitives (CPU evaluation for voxel carving)
// ---------------------------------------------------------------------------

/// Signed distance to a sphere of given `radius` centred at `center`.
pub fn sdf_sphere(point: Vec3, center: Vec3, radius: f32) -> f32 {
    (point - center).length() - radius
}

/// Signed distance to an axis-aligned box with `half_extents` centred at `center`.
pub fn sdf_box(point: Vec3, center: Vec3, half_extents: Vec3) -> f32 {
    let d = (point - center).abs() - half_extents;
    d.max(Vec3::ZERO).length() + d.x.max(d.y.max(d.z)).min(0.0)
}

/// Signed distance to an infinite plane with normal `n` (must be unit) passing
/// through `point_on_plane`.
pub fn sdf_plane(point: Vec3, normal: Vec3, point_on_plane: Vec3) -> f32 {
    (point - point_on_plane).dot(normal)
}

// ---------------------------------------------------------------------------
// Cut request
// ---------------------------------------------------------------------------

/// Describes a single cut operation to apply to the SVO.
#[derive(Clone, Debug)]
pub struct CutRequest {
    /// World-space center of the cutting tool.
    pub center: Vec3,
    /// Radius of the cutting sphere.
    pub radius: f32,
    /// Blend radius for smooth subtraction (0 = hard cut).
    pub blend: f32,
    /// Direction of the cut (used for particle emission and impact normal).
    pub direction: Vec3,
    /// Strength multiplier for particle/glass effects.
    pub impact_strength: f32,
}

// ---------------------------------------------------------------------------
// Cut particle system
// ---------------------------------------------------------------------------

/// A single debris particle emitted from a cut surface.
#[derive(Clone, Debug)]
pub struct CutParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub color: [u8; 4],
    pub lifetime: f32,
    pub age: f32,
    pub scale: f32,
}

/// Bevy resource holding the active cut-debris particle pool.
#[derive(Resource, Default)]
pub struct CutParticles {
    pub particles: Vec<CutParticle>,
}

/// Maximum number of simultaneous cut debris particles.
pub const MAX_CUT_PARTICLES: usize = 4096;

/// Emit debris particles along the cut surface.
///
/// Particles are distributed in a hemisphere around the impact point,
/// with velocity aimed outward from the cut normal.
pub fn emit_cut_particles(
    pool: &mut CutParticles,
    request: &CutRequest,
    material_color: [u8; 4],
    count: u32,
) {
    // Simple deterministic emission pattern (no randomness — reproducible for tests)
    for i in 0..count {
        if pool.particles.len() >= MAX_CUT_PARTICLES {
            break;
        }
        let t = i as f32 / count.max(1) as f32;
        let angle = t * std::f32::consts::TAU;
        // Distribute on hemisphere perpendicular to cut direction
        let tangent = if request.direction.y.abs() < 0.99 {
            request.direction.cross(Vec3::Y).normalize()
        } else {
            request.direction.cross(Vec3::X).normalize()
        };
        let bitangent = request.direction.cross(tangent);
        let spread = tangent * angle.cos() + bitangent * angle.sin();
        let velocity = (request.direction + spread * 0.5).normalize()
            * request.impact_strength
            * 2.0;

        pool.particles.push(CutParticle {
            position: request.center + spread * request.radius * 0.5,
            velocity,
            color: material_color,
            lifetime: 1.5,
            age: 0.0,
            scale: request.radius * 0.08,
        });
    }
}

/// Tick all cut particles forward by `dt` seconds.
/// Removes expired particles.
pub fn tick_cut_particles(pool: &mut CutParticles, dt: f32) {
    for p in &mut pool.particles {
        p.age += dt;
        p.velocity.y -= 9.81 * dt; // gravity
        p.position += p.velocity * dt;
        p.scale *= (1.0 - dt * 0.5).max(0.0); // shrink
    }
    pool.particles.retain(|p| p.age < p.lifetime);
}

// ---------------------------------------------------------------------------
// Liquid glass impact
// ---------------------------------------------------------------------------

/// A splash-like glass impact that produces refraction ripples.
#[derive(Clone, Debug)]
pub struct GlassImpact {
    pub center: Vec3,
    pub normal: Vec3,
    pub radius: f32,
    pub strength: f32,
    pub age: f32,
    pub lifetime: f32,
}

/// Bevy resource holding active glass impact events.
#[derive(Resource, Default)]
pub struct GlassImpacts {
    pub impacts: Vec<GlassImpact>,
}

/// Maximum simultaneous glass impacts.
pub const MAX_GLASS_IMPACTS: usize = 32;

/// Spawn a glass impact at the cut location.
pub fn spawn_glass_impact(impacts: &mut GlassImpacts, request: &CutRequest) {
    if impacts.impacts.len() >= MAX_GLASS_IMPACTS {
        // Remove oldest
        impacts.impacts.remove(0);
    }
    impacts.impacts.push(GlassImpact {
        center: request.center,
        normal: request.direction.normalize_or_zero(),
        radius: request.radius * 1.5,
        strength: request.impact_strength,
        age: 0.0,
        lifetime: 0.8,
    });
}

/// Tick glass impacts, removing expired ones.
pub fn tick_glass_impacts(impacts: &mut GlassImpacts, dt: f32) {
    for imp in &mut impacts.impacts {
        imp.age += dt;
        // Expand radius over time for ripple effect
        imp.radius += dt * 2.0;
        // Fade strength
        imp.strength *= (1.0 - dt * 1.5).max(0.0);
    }
    impacts.impacts.retain(|imp| imp.age < imp.lifetime);
}

// ---------------------------------------------------------------------------
// CSG Cut — applies to SVO
// ---------------------------------------------------------------------------

/// Apply a CSG subtraction cut to the voxel world.
///
/// Iterates over the bounding box of the cut sphere, evaluates the SDF, and
/// removes voxels that fall inside the subtracted region. Returns the number
/// of voxels removed and the average material colour of removed voxels (for
/// particle tinting).
pub fn apply_sdf_cut(
    world: &mut crate::svo::VoxelWorld,
    request: &CutRequest,
) -> (u32, [u8; 4]) {
    let half = request.radius.ceil() as i32 + 1;
    let cx = request.center.x.round() as i32;
    let cy = request.center.y.round() as i32;
    let cz = request.center.z.round() as i32;

    let mut removed = 0u32;
    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;

    for x in (cx - half)..=(cx + half) {
        for y in (cy - half)..=(cy + half) {
            for z in (cz - half)..=(cz + half) {
                let pos = IVec3::new(x, y, z);
                let world_pos = Vec3::new(x as f32, y as f32, z as f32);
                let dist = sdf_sphere(world_pos, request.center, request.radius);

                if dist < 0.0 {
                    // Voxel is inside the cutting sphere — remove it
                    if let Some(idx) = world.descend_to(pos) {
                        let packed = world.nodes[idx].color_data;
                        if packed != 0 {
                            let mat = crate::svo::VoxelMaterial::unpack(packed);
                            r_sum += mat.r as u64;
                            g_sum += mat.g as u64;
                            b_sum += mat.b as u64;
                            removed += 1;
                        }
                    }
                    world.remove_voxel(pos);
                }
            }
        }
    }

    let avg_color = if removed > 0 {
        [
            (r_sum / removed as u64) as u8,
            (g_sum / removed as u64) as u8,
            (b_sum / removed as u64) as u8,
            255,
        ]
    } else {
        [128, 128, 128, 255]
    };

    (removed, avg_color)
}

// ---------------------------------------------------------------------------
// Bevy systems
// ---------------------------------------------------------------------------

/// Per-frame system: tick cut particles with delta time.
fn tick_cut_particles_system(time: Res<Time>, mut pool: ResMut<CutParticles>) {
    tick_cut_particles(&mut pool, time.delta_secs());
}

/// Per-frame system: tick glass impacts with delta time.
fn tick_glass_impacts_system(time: Res<Time>, mut impacts: ResMut<GlassImpacts>) {
    tick_glass_impacts(&mut impacts, time.delta_secs());
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Registers SDF cutting systems and resources.
pub struct SdfCuttingPlugin;

impl Plugin for SdfCuttingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CutParticles>();
        app.init_resource::<GlassImpacts>();
        app.add_systems(Update, (tick_cut_particles_system, tick_glass_impacts_system));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csg_subtraction_basic() {
        // Point inside A (d_a = -1) and inside B (d_b = -0.5)
        // subtraction(A,B) = max(-1, -(-0.5)) = max(-1, 0.5) = 0.5 → outside
        let result = sdf_csg(CsgOp::Subtraction, -1.0, -0.5);
        assert!((result - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn csg_union_takes_minimum() {
        assert!((sdf_csg(CsgOp::Union, 2.0, 3.0) - 2.0).abs() < f32::EPSILON);
        assert!((sdf_csg(CsgOp::Union, -1.0, 1.0) - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn csg_intersection_takes_maximum() {
        assert!((sdf_csg(CsgOp::Intersection, 2.0, 3.0) - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn smooth_subtraction_matches_hard_at_zero_k() {
        let hard = sdf_csg(CsgOp::Subtraction, -1.0, -0.5);
        let smooth = sdf_smooth_subtraction(-1.0, -0.5, 0.0);
        assert!((hard - smooth).abs() < f32::EPSILON);
    }

    #[test]
    fn sdf_sphere_center_is_negative_radius() {
        let d = sdf_sphere(Vec3::ZERO, Vec3::ZERO, 2.0);
        assert!((d - (-2.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn sdf_sphere_surface_is_zero() {
        let d = sdf_sphere(Vec3::new(3.0, 0.0, 0.0), Vec3::ZERO, 3.0);
        assert!(d.abs() < f32::EPSILON);
    }

    #[test]
    fn sdf_box_center_is_negative() {
        let d = sdf_box(Vec3::ZERO, Vec3::ZERO, Vec3::splat(1.0));
        assert!(d < 0.0, "center of box should be inside (negative)");
    }

    #[test]
    fn sdf_box_outside_is_positive() {
        let d = sdf_box(Vec3::new(5.0, 0.0, 0.0), Vec3::ZERO, Vec3::splat(1.0));
        assert!(d > 0.0, "point far from box should be outside (positive)");
    }

    #[test]
    fn cut_particles_expire() {
        let mut pool = CutParticles::default();
        let req = CutRequest {
            center: Vec3::ZERO,
            radius: 1.0,
            blend: 0.0,
            direction: Vec3::Y,
            impact_strength: 1.0,
        };
        emit_cut_particles(&mut pool, &req, [200, 100, 50, 255], 10);
        assert_eq!(pool.particles.len(), 10);

        // Tick past lifetime
        tick_cut_particles(&mut pool, 2.0);
        assert!(pool.particles.is_empty(), "all particles should have expired");
    }

    #[test]
    fn glass_impacts_expire() {
        let mut impacts = GlassImpacts::default();
        let req = CutRequest {
            center: Vec3::ZERO,
            radius: 1.0,
            blend: 0.0,
            direction: Vec3::Y,
            impact_strength: 2.0,
        };
        spawn_glass_impact(&mut impacts, &req);
        assert_eq!(impacts.impacts.len(), 1);
        assert!((impacts.impacts[0].strength - 2.0).abs() < f32::EPSILON);

        tick_glass_impacts(&mut impacts, 1.0);
        assert!(impacts.impacts.is_empty(), "impact should have expired");
    }

    #[test]
    fn emit_respects_max_cap() {
        let mut pool = CutParticles::default();
        let req = CutRequest {
            center: Vec3::ZERO,
            radius: 1.0,
            blend: 0.0,
            direction: Vec3::Y,
            impact_strength: 1.0,
        };
        emit_cut_particles(&mut pool, &req, [255; 4], MAX_CUT_PARTICLES as u32 + 100);
        assert_eq!(pool.particles.len(), MAX_CUT_PARTICLES);
    }

    #[test]
    fn apply_sdf_cut_removes_voxels() {
        let mut world = crate::svo::VoxelWorld::new(8);
        let mat = crate::svo::VoxelMaterial::new(200, 100, 50, 128);
        // Place a single voxel at origin
        world.set_voxel(IVec3::ZERO, mat);

        let req = CutRequest {
            center: Vec3::ZERO,
            radius: 2.0,
            blend: 0.0,
            direction: Vec3::Y,
            impact_strength: 1.0,
        };
        let (removed, color) = apply_sdf_cut(&mut world, &req);
        assert!(removed >= 1, "should remove at least the placed voxel");
        // Verify average colour is reasonable
        assert!(color[0] > 0);
    }

    #[test]
    fn smooth_union_is_symmetric() {
        let a = sdf_smooth_union(1.0, 2.0, 0.5);
        let b = sdf_smooth_union(2.0, 1.0, 0.5);
        assert!((a - b).abs() < 1e-5, "smooth union should be commutative");
    }
}
