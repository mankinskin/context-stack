use super::*;

/// Plugin registering world generation resources.
pub struct WorldGenPlugin;

impl Plugin for WorldGenPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<WorldGenerator>();
        app.init_resource::<RegrowthTracker>();
    }
}
