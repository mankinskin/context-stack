use super::*;

/// Plugin registering SVO LOD management and GPU streaming systems.
pub struct SvoLodPlugin;

impl Plugin for SvoLodPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.init_resource::<LodParams>();
        app.insert_resource(ChunkLruCache::new(64 * 1024 * 1024));
        app.insert_resource(LodStreamingState::new(4));
        app.init_resource::<GlassOccluders>();

        app.add_systems(Update, (lod_propagation_system, lod_streaming_system));
    }
}
