use super::*;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<AttackState>();
        app.init_resource::<DamageNumberPool>();
        app.init_resource::<BattleGlassEffect>();
        app.init_resource::<RespawnState>();
        app.init_resource::<PlayerVitals>();

        app.add_systems(
            Update,
            (
                attack_cooldown_system,
                process_combat_events_system,
                damage_number_system,
                battle_glass_system,
                vitals_sync_system,
                respawn_system,
            ),
        );
    }
}
