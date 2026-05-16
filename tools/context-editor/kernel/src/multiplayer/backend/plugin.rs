use super::*;

/// Plugin registering multiplayer backend types and fallback tick systems.
pub struct MultiplayerBackendPlugin;

impl Plugin for MultiplayerBackendPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<MultiplayerConnection>();
        app.init_resource::<PlayerTable>();
        app.init_resource::<BlueprintTable>();
        app.init_resource::<InventoryTable>();
        app.init_resource::<ReducerQueue>();
        app.init_resource::<CombatEventQueue>();
        app.init_resource::<LocalGameTick>();

        let identity = PlayerIdentity::local();
        let mut player_table = PlayerTable::default();
        player_table.players.insert(identity, PlayerRow::default());
        app.insert_resource(player_table);

        app.add_systems(
            Update,
            (local_tick_system, drain_reducer_queue_system),
        );
    }
}
