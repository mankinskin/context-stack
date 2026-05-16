use super::*;

#[test]
fn ray_march_uniform_size() {
    assert_eq!(std::mem::size_of::<RayMarchUniformData>(), 224);
}

#[test]
fn dispatch_dimensions() {
    let (width, height) = (1920u32, 1080u32);
    let wg_x = (width + 7) / 8;
    let wg_y = (height + 7) / 8;
    assert_eq!(wg_x, 240);
    assert_eq!(wg_y, 135);
    assert!(wg_x * 8 >= width);
    assert!(wg_y * 8 >= height);
}
