use super::*;

#[test]
fn spell_type_from_u8_valid() {
    assert_eq!(SpellType::from_u8(0), Some(SpellType::Fireball));
    assert_eq!(SpellType::from_u8(4), Some(SpellType::Lightning));
}

#[test]
fn spell_type_from_u8_invalid() {
    assert_eq!(SpellType::from_u8(5), None);
    assert_eq!(SpellType::from_u8(255), None);
}

#[test]
fn mana_cost_scales_with_power() {
    let cost_low = SpellType::Fireball.mana_cost(0.5);
    let cost_high = SpellType::Fireball.mana_cost(2.0);
    assert!(cost_high > cost_low);
}

#[test]
fn mana_cost_minimum_clamp() {
    let cost = SpellType::Fireball.mana_cost(0.1);
    let expected = (15.0 * 0.5f32).ceil() as i32;
    assert_eq!(cost, expected);
}

#[test]
fn base_radius_varies_by_type() {
    assert!(
        SpellType::Gravity.base_radius() > SpellType::Fireball.base_radius()
    );
    assert!(
        SpellType::Shield.base_radius() > SpellType::Lightning.base_radius()
    );
}

#[test]
fn fireball_is_projectile() {
    assert!(SpellType::Fireball.is_projectile());
    assert!(!SpellType::Shield.is_projectile());
    assert!(!SpellType::Frost.is_projectile());
}

#[test]
fn lightning_is_projectile() {
    assert!(SpellType::Lightning.is_projectile());
}

#[test]
fn shield_not_projectile() {
    assert!(!SpellType::Shield.is_projectile());
    assert_eq!(SpellType::Shield.speed(), 0.0);
}

#[test]
fn active_spell_expires() {
    let mut spell = ActiveSpell::from_server(
        1,
        PlayerIdentity::local(),
        SpellType::Fireball,
        Vec3::ZERO,
        Vec3::X,
        1.0,
        3.0,
    );
    assert!(!spell.is_expired());
    spell.elapsed = 3.0;
    assert!(spell.is_expired());
}

#[test]
fn active_spell_to_sdf_fireball() {
    let spell = ActiveSpell::from_server(
        1,
        PlayerIdentity::local(),
        SpellType::Fireball,
        Vec3::new(1.0, 2.0, 3.0),
        Vec3::X,
        1.0,
        3.0,
    );
    let sdf = spell.to_sdf();
    assert!(matches!(sdf, SpellSdf::Fireball { .. }));
}

#[test]
fn active_spell_to_sdf_shield() {
    let spell = ActiveSpell::from_server(
        2,
        PlayerIdentity::local(),
        SpellType::Shield,
        Vec3::ZERO,
        Vec3::Y,
        1.5,
        SHIELD_DURATION,
    );
    let sdf = spell.to_sdf();
    assert!(matches!(sdf, SpellSdf::Shield { .. }));
}

#[test]
fn sdf_evaluate_fireball_inside() {
    let sdf = SpellSdf::Fireball {
        center: Vec3::ZERO,
        radius: 2.0,
        noise_scale: 5.0,
        emission_color: Vec3::ONE,
        emission_intensity: 3.0,
    };
    assert!(sdf.evaluate(Vec3::ZERO) < 0.0);
}

#[test]
fn sdf_evaluate_fireball_outside() {
    let sdf = SpellSdf::Fireball {
        center: Vec3::ZERO,
        radius: 1.0,
        noise_scale: 5.0,
        emission_color: Vec3::ONE,
        emission_intensity: 3.0,
    };
    assert!(sdf.evaluate(Vec3::new(5.0, 0.0, 0.0)) > 0.0);
}

#[test]
fn sdf_evaluate_shield_shell() {
    let sdf = SpellSdf::Shield {
        center: Vec3::ZERO,
        outer_radius: 2.0,
        thickness: 0.1,
        ior: 1.3,
        tint: Vec3::ONE,
    };
    assert!(sdf.evaluate(Vec3::ZERO) > 0.0);
    let on_shell = sdf.evaluate(Vec3::new(2.0, 0.0, 0.0));
    assert!(on_shell.abs() < 0.2);
}

#[test]
fn sdf_evaluate_gravity() {
    let sdf = SpellSdf::Gravity {
        center: Vec3::new(5.0, 5.0, 5.0),
        radius: 3.0,
        strength: 1.0,
    };
    assert!(sdf.evaluate(Vec3::ZERO) > 0.0);
    assert!(sdf.evaluate(Vec3::new(5.0, 5.0, 5.0)) < 0.0);
}

#[test]
fn sdf_evaluate_lightning_on_beam() {
    let sdf = SpellSdf::Lightning {
        origin: Vec3::ZERO,
        direction: Vec3::X,
        length: 10.0,
        radius: 0.5,
        branch_noise: 0.1,
    };
    assert!(sdf.evaluate(Vec3::new(5.0, 0.0, 0.0)) < 0.0);
    assert!(sdf.evaluate(Vec3::new(5.0, 10.0, 0.0)) > 0.0);
}

#[test]
fn spell_cooldowns_can_cast() {
    let cd = SpellCooldowns::default();
    assert!(cd.can_cast(SpellType::Fireball));
}

#[test]
fn spell_cooldowns_trigger_blocks_cast() {
    let mut cd = SpellCooldowns::default();
    cd.trigger(SpellType::Fireball);
    assert!(!cd.can_cast(SpellType::Fireball));
    assert!(!cd.can_cast(SpellType::Shield));
}

#[test]
fn spell_cooldowns_tick_recovers() {
    let mut cd = SpellCooldowns::default();
    cd.trigger(SpellType::Fireball);
    cd.tick(GLOBAL_COOLDOWN + 0.01);
    assert!(cd.global_remaining <= 0.0);
}

#[test]
fn spell_cooldowns_full_recovery() {
    let mut cd = SpellCooldowns::default();
    cd.trigger(SpellType::Shield);
    cd.tick(SHIELD_DURATION);
    assert!(cd.can_cast(SpellType::Shield));
}

#[test]
fn gpu_spell_data_size() {
    assert_eq!(std::mem::size_of::<GpuSpellData>(), 80);
}

#[test]
fn gpu_spell_pack_roundtrip() {
    let spell = ActiveSpell::from_server(
        7,
        PlayerIdentity::new(99),
        SpellType::Fireball,
        Vec3::new(1.0, 2.0, 3.0),
        Vec3::Z,
        1.5,
        3.0,
    );
    let packed = SpellBuffer::pack_spell(&spell);
    assert_eq!(packed.spell_type, 0);
    assert_eq!(packed.pos, [1.0, 2.0, 3.0]);
    assert!(packed.power > 1.0);
}

#[test]
fn emission_colors_distinct() {
    let fb = SpellType::Fireball.emission_color();
    let sh = SpellType::Shield.emission_color();
    assert!(fb != sh);
}

#[test]
fn sd_capsule_line_on_axis() {
    let d = sd_capsule_line(
        Vec3::new(5.0, 0.0, 0.0),
        Vec3::ZERO,
        Vec3::new(10.0, 0.0, 0.0),
        0.5,
    );
    assert!(d < 0.0);
}

#[test]
fn sd_capsule_line_far_away() {
    let d = sd_capsule_line(
        Vec3::new(5.0, 100.0, 0.0),
        Vec3::ZERO,
        Vec3::new(10.0, 0.0, 0.0),
        0.5,
    );
    assert!(d > 0.0);
}
