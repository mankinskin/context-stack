use super::*;

#[test]
fn depth_bridge_uniform_size() {
    assert_eq!(std::mem::size_of::<DepthBridgeUniformData>(), 32);
}
