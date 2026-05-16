use super::*;

pub struct SkillPlugin;

impl Plugin for SkillPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<SpellCooldowns>();
        app.init_resource::<SpellBuffer>();
        app.add_systems(
            Update,
            (cooldown_system, advance_spells_system, spell_buffer_system),
        );
    }
}
