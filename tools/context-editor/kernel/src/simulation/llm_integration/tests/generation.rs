use super::super::*;

#[test]
fn generation_mode_from_u8_valid() {
    assert_eq!(
        GenerationMode::from_u8(0),
        Some(GenerationMode::VoxelStructure)
    );
    assert_eq!(
        GenerationMode::from_u8(1),
        Some(GenerationMode::ShaderEffect)
    );
    assert_eq!(
        GenerationMode::from_u8(2),
        Some(GenerationMode::SpellModifier)
    );
}

#[test]
fn generation_mode_from_u8_invalid() {
    assert_eq!(GenerationMode::from_u8(3), None);
    assert_eq!(GenerationMode::from_u8(255), None);
}

#[test]
fn generation_mode_system_prompt() {
    let voxel = GenerationMode::VoxelStructure;
    assert!(voxel.system_prompt_header().contains("voxel_structure"));

    let shader = GenerationMode::ShaderEffect;
    assert!(shader.system_prompt_header().contains("shader_effect"));
}

#[test]
fn validate_voxels_empty() {
    let result = validate_voxels(&[], (0.0, 0.0, 0.0));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}

#[test]
fn validate_voxels_single_valid() {
    let voxels = [[0, 5, -3, 0xFF0000FF]];
    let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
    assert!(result.is_ok());
    let generated = &result.unwrap()[0];
    assert_eq!(generated.x, 0);
    assert_eq!(generated.y, 5);
    assert_eq!(generated.z, -3);
    assert_eq!(generated.color, 0xFF0000FF);
}

#[test]
fn validate_voxels_at_boundary() {
    let voxels = [[64, 0, 0, 0xFFFFFFFF]];
    let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
    assert!(result.is_ok());
}

#[test]
fn validate_voxels_out_of_coordinate_range() {
    let voxels = [[65, 0, 0, 0xFF]];
    let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("out of range"));
}

#[test]
fn validate_voxels_negative_out_of_range() {
    let voxels = [[0, -65, 0, 0xFF]];
    let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
    assert!(result.is_err());
}

#[test]
fn validate_voxels_too_far_from_player() {
    let voxels = [[64, 64, 0, 0xFF]];
    let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("units from player"));
}

#[test]
fn validate_voxels_near_player_pos() {
    let voxels = [[62, 0, 0, 0xFF]];
    let result = validate_voxels(&voxels, (60.0, 0.0, 0.0));
    assert!(result.is_ok());
}

#[test]
fn validate_voxels_too_many() {
    let voxels: Vec<[i64; 4]> =
        (0..4097).map(|i| [0, 0, i % 64, 0xFF]).collect();
    let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Too many voxels"));
}

#[test]
fn validate_voxels_exactly_max() {
    let voxels: Vec<[i64; 4]> = (0..MAX_GENERATED_VOXELS)
        .map(|i| {
            let x = (i % 11) as i64 - 5;
            let y = ((i / 11) % 11) as i64 - 5;
            let z = ((i / 121) % 11) as i64 - 5;
            [x, y, z, 0xAABBCCDD]
        })
        .collect();
    let result = validate_voxels(&voxels, (0.0, 0.0, 0.0));
    assert!(result.is_ok());
}
