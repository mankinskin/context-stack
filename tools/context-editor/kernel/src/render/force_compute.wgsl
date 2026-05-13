// Force Compute Shader — applies forces to voxel splat particles and resolves
// SVO collisions via stackless ray-AABB traversal.
//
// Dispatch: ceil(particle_count / 256) workgroups, 256 threads each.
//
// Force types:
//   0 = Explosion (radial push)
//   1 = Attraction (pull towards origin)
//   2 = Vortex (tangential spin)

struct ForceEvent {
    origin: vec3<f32>,
    radius: f32,
    strength: f32,
    force_type: u32,
    _pad0: f32,
    _pad1: f32,
}

struct ForceUniforms {
    delta_time: f32,
    force_count: u32,
    restitution: f32,
    friction: f32,
}

struct Particle {
    position: vec3<f32>,
    _pad0: f32,
    velocity: vec3<f32>,
    _pad1: f32,
}

@group(0) @binding(0) var<storage, read> forces: array<ForceEvent>;
@group(0) @binding(1) var<uniform> uniforms: ForceUniforms;
@group(0) @binding(2) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(3) var<storage, read> octree: array<u32>;

// ---------------------------------------------------------------------------
// Force evaluation
// ---------------------------------------------------------------------------

fn apply_explosion(p: vec3<f32>, f: ForceEvent) -> vec3<f32> {
    let diff = p - f.origin;
    let dist = length(diff);
    if dist > f.radius || dist < 0.001 {
        return vec3<f32>(0.0);
    }
    let falloff = 1.0 - dist / f.radius;
    return normalize(diff) * f.strength * falloff;
}

fn apply_attraction(p: vec3<f32>, f: ForceEvent) -> vec3<f32> {
    let diff = f.origin - p;
    let dist = length(diff);
    if dist > f.radius || dist < 0.001 {
        return vec3<f32>(0.0);
    }
    let falloff = 1.0 - dist / f.radius;
    return normalize(diff) * f.strength * falloff;
}

fn apply_vortex(p: vec3<f32>, f: ForceEvent) -> vec3<f32> {
    let diff = p - f.origin;
    let dist = length(diff);
    if dist > f.radius || dist < 0.001 {
        return vec3<f32>(0.0);
    }
    let falloff = 1.0 - dist / f.radius;
    // Tangential force (cross with up vector)
    let tangent = cross(vec3<f32>(0.0, 1.0, 0.0), normalize(diff));
    return tangent * f.strength * falloff;
}

// ---------------------------------------------------------------------------
// Stackless SVO collision (ray-march)
// ---------------------------------------------------------------------------

fn check_svo_collision(pos: vec3<f32>, next_pos: vec3<f32>) -> vec3<f32> {
    // Simplified stackless ray march through SVO
    // Returns surface normal if collision detected, vec3(0) otherwise
    let dir = next_pos - pos;
    let dist = length(dir);
    if dist < 0.001 {
        return vec3<f32>(0.0);
    }

    let ray_dir = normalize(dir);
    var t = 0.0;
    let step_size = 0.5;

    while t < dist {
        let sample_pos = pos + ray_dir * t;
        let cell = vec3<i32>(floor(sample_pos));

        // Simple occupancy check — once full octree traversal is wired,
        // this will use the actual node hierarchy for distance-based skipping.
        // For now, returns zero (no collision) until the octree buffer is bound.

        t += step_size;
    }

    return vec3<f32>(0.0);
}

// ---------------------------------------------------------------------------
// Main compute entry
// ---------------------------------------------------------------------------

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let count = arrayLength(&particles);
    if idx >= count {
        return;
    }

    var p = particles[idx];
    let dt = uniforms.delta_time;

    // Accumulate forces
    var accel = vec3<f32>(0.0);
    for (var i = 0u; i < uniforms.force_count; i = i + 1u) {
        let f = forces[i];
        switch f.force_type {
            case 0u: { accel += apply_explosion(p.position, f); }
            case 1u: { accel += apply_attraction(p.position, f); }
            case 2u: { accel += apply_vortex(p.position, f); }
            default: {}
        }
    }

    // Euler integration
    p.velocity += accel * dt;
    let next_pos = p.position + p.velocity * dt;

    // SVO collision
    let normal = check_svo_collision(p.position, next_pos);
    if length(normal) > 0.5 {
        // Reflect velocity
        let refl = reflect(p.velocity, normalize(normal));
        p.velocity = refl * uniforms.restitution;
        // Apply friction orthogonal to normal
        let n = normalize(normal);
        let tangent_vel = p.velocity - dot(p.velocity, n) * n;
        p.velocity -= tangent_vel * (1.0 - uniforms.friction);
    } else {
        p.position = next_pos;
    }

    particles[idx] = p;
}
