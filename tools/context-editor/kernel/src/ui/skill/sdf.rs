use bevy::prelude::Vec3;

/// Spell SDF variants with typed parameters.
#[derive(Clone, Debug)]
pub enum SpellSdf {
    Fireball {
        center: Vec3,
        radius: f32,
        noise_scale: f32,
        emission_color: Vec3,
        emission_intensity: f32,
    },
    Shield {
        center: Vec3,
        outer_radius: f32,
        thickness: f32,
        ior: f32,
        tint: Vec3,
    },
    Frost {
        center: Vec3,
        radius: f32,
        roughness_boost: f32,
    },
    Gravity {
        center: Vec3,
        radius: f32,
        strength: f32,
    },
    Lightning {
        origin: Vec3,
        direction: Vec3,
        length: f32,
        radius: f32,
        branch_noise: f32,
    },
}

impl SpellSdf {
    /// Sample the signed distance at point `p`.
    pub fn evaluate(
        &self,
        p: Vec3,
    ) -> f32 {
        match self {
            SpellSdf::Fireball { center, radius, .. } =>
                (p - *center).length() - radius,
            SpellSdf::Shield {
                center,
                outer_radius,
                thickness,
                ..
            } => {
                let d_outer = (p - *center).length() - outer_radius;
                let d_inner =
                    (p - *center).length() - (outer_radius - thickness);
                d_outer.max(-d_inner)
            },
            SpellSdf::Frost { center, radius, .. } =>
                (p - *center).length() - radius,
            SpellSdf::Gravity { center, radius, .. } =>
                (p - *center).length() - radius,
            SpellSdf::Lightning {
                origin,
                direction,
                length,
                radius,
                ..
            } => {
                let dir = direction.normalize_or_zero();
                let end = *origin + dir * *length;
                sd_capsule_line(p, *origin, end, *radius)
            },
        }
    }
}

/// Signed distance from point to a capsule (line segment + radius).
pub(crate) fn sd_capsule_line(
    p: Vec3,
    a: Vec3,
    b: Vec3,
    radius: f32,
) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(ba) / ba.length_squared()).clamp(0.0, 1.0);
    (pa - ba * h).length() - radius
}
