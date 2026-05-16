use super::*;

/// Plugin registering multiplayer networking systems.
pub struct MultiplayerNetPlugin;

impl Plugin for MultiplayerNetPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<NetworkConnection>();
        app.init_resource::<ChunkSubscriptionManager>();
        app.init_resource::<DeltaQueue>();
        app.init_resource::<OfflineMutationQueue>();

        app.add_systems(
            Update,
            (
                update_subscriptions_system,
                apply_server_deltas_system,
                bandwidth_tracking_system,
            ),
        );
    }
}
