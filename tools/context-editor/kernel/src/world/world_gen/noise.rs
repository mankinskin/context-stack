/// Simple hash-based pseudo-random noise (deterministic from seed).
///
/// Produces values in [-1.0, 1.0]. Not a true Simplex implementation but
/// shares the same API and determinism guarantees for procedural generation.
pub fn noise3d(
    seed: u64,
    x: f32,
    y: f32,
    z: f32,
) -> f32 {
    let ix = (x.floor() as i32) as u64;
    let iy = (y.floor() as i32) as u64;
    let iz = (z.floor() as i32) as u64;

    let fx = x - x.floor();
    let fy = y - y.floor();
    let fz = z - z.floor();

    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let uz = fz * fz * (3.0 - 2.0 * fz);

    let v000 = hash_to_float(seed, ix, iy, iz);
    let v100 = hash_to_float(seed, ix.wrapping_add(1), iy, iz);
    let v010 = hash_to_float(seed, ix, iy.wrapping_add(1), iz);
    let v110 = hash_to_float(seed, ix.wrapping_add(1), iy.wrapping_add(1), iz);
    let v001 = hash_to_float(seed, ix, iy, iz.wrapping_add(1));
    let v101 = hash_to_float(seed, ix.wrapping_add(1), iy, iz.wrapping_add(1));
    let v011 = hash_to_float(seed, ix, iy.wrapping_add(1), iz.wrapping_add(1));
    let v111 = hash_to_float(
        seed,
        ix.wrapping_add(1),
        iy.wrapping_add(1),
        iz.wrapping_add(1),
    );

    let x00 = lerp(v000, v100, ux);
    let x10 = lerp(v010, v110, ux);
    let x01 = lerp(v001, v101, ux);
    let x11 = lerp(v011, v111, ux);
    let y0 = lerp(x00, x10, uy);
    let y1 = lerp(x01, x11, uy);
    lerp(y0, y1, uz)
}

/// 2D noise (for biome selection).
pub fn noise2d(
    seed: u64,
    x: f32,
    z: f32,
) -> f32 {
    noise3d(seed, x, 0.0, z)
}

/// Fractal Brownian Motion - layered noise for terrain detail.
pub fn fbm(
    seed: u64,
    x: f32,
    y: f32,
    z: f32,
    octaves: u32,
) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut frequency = 1.0f32;
    let mut max_amp = 0.0f32;

    for i in 0..octaves {
        value += noise3d(
            seed.wrapping_add(i as u64 * 31337),
            x * frequency,
            y * frequency,
            z * frequency,
        ) * amplitude;
        max_amp += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    if max_amp > 0.0 {
        value / max_amp
    } else {
        0.0
    }
}

fn hash_to_float(
    seed: u64,
    x: u64,
    y: u64,
    z: u64,
) -> f32 {
    let h = hash_combine(seed, x, y, z);
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn hash_combine(
    seed: u64,
    x: u64,
    y: u64,
    z: u64,
) -> u32 {
    let mut h = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(x.wrapping_mul(1442695040888963407))
        .wrapping_add(y.wrapping_mul(2862933555777941757))
        .wrapping_add(z.wrapping_mul(3037000499));
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    (h & 0xFFFFFFFF) as u32
}

fn lerp(
    a: f32,
    b: f32,
    t: f32,
) -> f32 {
    a + (b - a) * t
}
