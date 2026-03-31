//! Theme Palette — centralised visual theming for all render elements.
//!
//! Implements the Style ticket: a single [`ThemePalette`] Bevy resource drives
//! SVO voxel materials, PBR parameters, glass tints, particles, and lighting.
//!
//! ## Data flow
//!
//! ```text
//! ThemePalette (Resource)
//!   ├─ MaterialDef ──▶ VoxelMaterial::pack() ──▶ OctreeNode.color_data
//!   │                                             └─▶ GPU splat pipeline unpacks per-pixel
//!   ├─ glass_tint ──▶ (future) glass panel uniform
//!   └─ key_light_color ──▶ (future) light uniform
//! ```

use bevy::prelude::*;
use std::collections::HashMap;

use crate::svo::{VoxelMaterial, VoxelWorld};

// ---------------------------------------------------------------------------
// MaterialRef — palette slot identifier
// ---------------------------------------------------------------------------

/// Names a slot in [`ThemePalette`], linking a voxel to a palette material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MaterialRef {
    Primary,
    Secondary,
    Terrain,
    Highlight,
}

// ---------------------------------------------------------------------------
// MaterialDef — high-level PBR material description
// ---------------------------------------------------------------------------

/// A PBR material definition stored in the palette.
///
/// Converted to [`VoxelMaterial`] (and its packed u32) when writing to the SVO,
/// or to Bevy's [`StandardMaterial`] for mesh rendering.
#[derive(Clone, Debug)]
pub struct MaterialDef {
    /// sRGB color components in \[0.0, 1.0\].
    pub base_color: [f32; 3],
    /// Perceptual roughness in \[0.0, 1.0\].
    pub roughness: f32,
    /// Metallic flag (dielectric vs metal).
    pub metallic: bool,
}

impl MaterialDef {
    /// Convert to a Bevy [`Color`] (sRGB).
    pub fn to_bevy_color(&self) -> Color {
        Color::srgb(self.base_color[0], self.base_color[1], self.base_color[2])
    }

    /// Convert to the SVO's compact [`VoxelMaterial`].
    pub fn to_voxel_material(&self) -> VoxelMaterial {
        VoxelMaterial {
            r: (self.base_color[0] * 255.0).clamp(0.0, 255.0) as u8,
            g: (self.base_color[1] * 255.0).clamp(0.0, 255.0) as u8,
            b: (self.base_color[2] * 255.0).clamp(0.0, 255.0) as u8,
            roughness: (self.roughness * 31.0).clamp(0.0, 31.0) as u8,
            metallic: self.metallic,
            sdf_type: 0,
        }
    }

    /// Pack directly into `OctreeNode::color_data` u32.
    pub fn pack(&self) -> u32 {
        self.to_voxel_material().pack()
    }

    /// Convert to a Bevy [`StandardMaterial`] for mesh rendering.
    pub fn to_standard_material(&self) -> StandardMaterial {
        StandardMaterial {
            base_color: self.to_bevy_color(),
            metallic: if self.metallic { 1.0 } else { 0.0 },
            perceptual_roughness: self.roughness,
            ..default()
        }
    }
}

// ---------------------------------------------------------------------------
// ThemePalette — central theme resource
// ---------------------------------------------------------------------------

/// Central theme resource driving all visual elements.
///
/// Changing any field triggers [`theme_update_svo`] which re-packs every
/// referenced SVO voxel and marks dirty ranges for GPU re-upload.
#[derive(Resource, Clone, Debug)]
pub struct ThemePalette {
    // Voxel materials
    pub voxel_primary: MaterialDef,
    pub voxel_secondary: MaterialDef,
    pub voxel_terrain: MaterialDef,
    pub voxel_highlight: MaterialDef,

    // Glass
    pub glass_tint: Color,
    pub glass_frosted_tint: Color,
    pub glass_accent: Color,

    // Particles
    pub particle_primary: Color,
    pub particle_secondary: Color,

    // Lighting
    pub ambient_color: Color,
    pub key_light_color: Color,
    pub fill_light_color: Color,

    // UI text
    pub text_primary: Color,
    pub text_secondary: Color,
}

impl ThemePalette {
    /// Dark cyberpunk theme — default.
    pub fn dark_default() -> Self {
        Self {
            voxel_primary: MaterialDef {
                base_color: [0.2, 0.7, 0.9],
                roughness: 0.35,
                metallic: false,
            },
            voxel_secondary: MaterialDef {
                base_color: [0.9, 0.3, 0.4],
                roughness: 0.5,
                metallic: false,
            },
            voxel_terrain: MaterialDef {
                base_color: [0.15, 0.18, 0.22],
                roughness: 0.8,
                metallic: false,
            },
            voxel_highlight: MaterialDef {
                base_color: [0.95, 0.85, 0.2],
                roughness: 0.2,
                metallic: true,
            },
            glass_tint: Color::srgba(0.0, 0.8, 0.9, 0.3),
            glass_frosted_tint: Color::srgba(0.5, 0.6, 0.7, 0.5),
            glass_accent: Color::srgba(0.0, 1.0, 0.8, 0.4),
            particle_primary: Color::srgb(0.0, 0.9, 1.0),
            particle_secondary: Color::srgb(1.0, 0.4, 0.1),
            ambient_color: Color::srgb(0.03, 0.04, 0.08),
            key_light_color: Color::srgb(1.0, 0.95, 0.85),
            fill_light_color: Color::srgb(0.3, 0.35, 0.5),
            text_primary: Color::srgb(0.9, 0.95, 1.0),
            text_secondary: Color::srgb(0.5, 0.55, 0.6),
        }
    }

    /// Light clean theme.
    pub fn light_default() -> Self {
        Self {
            voxel_primary: MaterialDef {
                base_color: [0.3, 0.5, 0.85],
                roughness: 0.4,
                metallic: false,
            },
            voxel_secondary: MaterialDef {
                base_color: [0.85, 0.4, 0.3],
                roughness: 0.45,
                metallic: false,
            },
            voxel_terrain: MaterialDef {
                base_color: [0.7, 0.72, 0.68],
                roughness: 0.7,
                metallic: false,
            },
            voxel_highlight: MaterialDef {
                base_color: [0.9, 0.7, 0.1],
                roughness: 0.15,
                metallic: true,
            },
            glass_tint: Color::srgba(0.6, 0.8, 0.9, 0.15),
            glass_frosted_tint: Color::srgba(0.8, 0.85, 0.9, 0.3),
            glass_accent: Color::srgba(0.2, 0.6, 0.9, 0.2),
            particle_primary: Color::srgb(0.2, 0.6, 0.9),
            particle_secondary: Color::srgb(0.9, 0.5, 0.2),
            ambient_color: Color::srgb(0.6, 0.62, 0.65),
            key_light_color: Color::srgb(1.0, 1.0, 0.98),
            fill_light_color: Color::srgb(0.6, 0.65, 0.8),
            text_primary: Color::srgb(0.1, 0.1, 0.12),
            text_secondary: Color::srgb(0.4, 0.42, 0.45),
        }
    }

    /// High-contrast accessibility theme.
    pub fn high_contrast() -> Self {
        Self {
            voxel_primary: MaterialDef {
                base_color: [0.0, 0.0, 1.0],
                roughness: 0.3,
                metallic: false,
            },
            voxel_secondary: MaterialDef {
                base_color: [1.0, 0.0, 0.0],
                roughness: 0.3,
                metallic: false,
            },
            voxel_terrain: MaterialDef {
                base_color: [0.1, 0.1, 0.1],
                roughness: 0.9,
                metallic: false,
            },
            voxel_highlight: MaterialDef {
                base_color: [1.0, 1.0, 0.0],
                roughness: 0.1,
                metallic: true,
            },
            glass_tint: Color::srgba(0.0, 1.0, 1.0, 0.5),
            glass_frosted_tint: Color::srgba(1.0, 1.0, 1.0, 0.6),
            glass_accent: Color::srgba(1.0, 0.0, 1.0, 0.5),
            particle_primary: Color::srgb(1.0, 1.0, 0.0),
            particle_secondary: Color::srgb(0.0, 1.0, 0.0),
            ambient_color: Color::srgb(0.08, 0.08, 0.08),
            key_light_color: Color::srgb(1.0, 1.0, 1.0),
            fill_light_color: Color::srgb(0.5, 0.5, 0.5),
            text_primary: Color::srgb(1.0, 1.0, 1.0),
            text_secondary: Color::srgb(0.8, 0.8, 0.0),
        }
    }

    /// Resolve a [`MaterialRef`] to the corresponding [`MaterialDef`].
    pub fn resolve(&self, mat_ref: MaterialRef) -> &MaterialDef {
        match mat_ref {
            MaterialRef::Primary => &self.voxel_primary,
            MaterialRef::Secondary => &self.voxel_secondary,
            MaterialRef::Terrain => &self.voxel_terrain,
            MaterialRef::Highlight => &self.voxel_highlight,
        }
    }
}

// ---------------------------------------------------------------------------
// MaterialRefMap — tracks which SVO nodes belong to which palette slot
// ---------------------------------------------------------------------------

/// Maps SVO node indices to palette slots for runtime re-theming.
#[derive(Resource, Default)]
pub struct MaterialRefMap {
    pub refs: HashMap<usize, MaterialRef>,
}

impl MaterialRefMap {
    /// Build the ref map by scanning all SVO leaf nodes and matching their
    /// packed `color_data` against the current palette materials.
    pub fn build_from_world(world: &VoxelWorld, palette: &ThemePalette) -> Self {
        let mut refs = HashMap::new();
        let slots = [
            (palette.voxel_primary.pack(), MaterialRef::Primary),
            (palette.voxel_secondary.pack(), MaterialRef::Secondary),
            (palette.voxel_terrain.pack(), MaterialRef::Terrain),
            (palette.voxel_highlight.pack(), MaterialRef::Highlight),
        ];
        for (idx, node) in world.nodes.iter().enumerate() {
            if !node.is_leaf() || node.color_data == 0 {
                continue;
            }
            for &(packed, mat_ref) in &slots {
                if node.color_data == packed {
                    refs.insert(idx, mat_ref);
                    break;
                }
            }
        }
        Self { refs }
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Re-packs every referenced SVO voxel when the palette changes.
///
/// Runs in `PostUpdate`, after input systems have toggled the palette.
/// Dirty ranges propagate through `svo_upload_system` → GPU next frame.
pub fn theme_update_svo(
    palette: Res<ThemePalette>,
    mut voxel_world: ResMut<VoxelWorld>,
    ref_map: Res<MaterialRefMap>,
) {
    if !palette.is_changed() {
        return;
    }
    for (&idx, &mat_ref) in ref_map.refs.iter() {
        if idx >= voxel_world.nodes.len() {
            continue;
        }
        let new_packed = palette.resolve(mat_ref).pack();
        if voxel_world.nodes[idx].color_data != new_packed {
            voxel_world.nodes[idx].color_data = new_packed;
            voxel_world.mark_dirty(idx);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svo::VoxelMaterial;

    #[test]
    fn material_def_pack_roundtrip() {
        let def = MaterialDef {
            base_color: [0.5, 0.25, 0.75],
            roughness: 0.5,
            metallic: false,
        };
        let packed = def.pack();
        let unpacked = VoxelMaterial::unpack(packed);
        // u8 quantisation loses precision; check within 1/255
        assert!((unpacked.r as f32 / 255.0 - 0.5).abs() < 0.01);
        assert!((unpacked.g as f32 / 255.0 - 0.25).abs() < 0.01);
        assert!((unpacked.b as f32 / 255.0 - 0.75).abs() < 0.01);
        assert!(!unpacked.metallic);
    }

    #[test]
    fn material_def_metallic_pack() {
        let def = MaterialDef {
            base_color: [0.95, 0.85, 0.2],
            roughness: 0.2,
            metallic: true,
        };
        let packed = def.pack();
        let unpacked = VoxelMaterial::unpack(packed);
        assert!(unpacked.metallic);
        assert!((unpacked.roughness as f32 / 31.0 - 0.2).abs() < 0.05);
    }

    #[test]
    fn dark_and_light_presets_differ() {
        let dark = ThemePalette::dark_default();
        let light = ThemePalette::light_default();
        assert_ne!(dark.voxel_primary.pack(), light.voxel_primary.pack());
        assert_ne!(dark.voxel_terrain.pack(), light.voxel_terrain.pack());
    }

    #[test]
    fn resolve_returns_correct_slot() {
        let palette = ThemePalette::dark_default();
        assert_eq!(
            palette.resolve(MaterialRef::Primary).pack(),
            palette.voxel_primary.pack()
        );
        assert_eq!(
            palette.resolve(MaterialRef::Highlight).pack(),
            palette.voxel_highlight.pack()
        );
    }

    #[test]
    fn theme_update_repacks_voxels() {
        let dark = ThemePalette::dark_default();
        let light = ThemePalette::light_default();

        let mut world = VoxelWorld::new(4);
        let mat = dark.voxel_primary.to_voxel_material();
        world.set_voxel(IVec3::new(2, 2, 2), mat);

        // Find the leaf node we just set
        let leaf_idx = world
            .nodes
            .iter()
            .position(|n| n.is_leaf() && n.color_data == dark.voxel_primary.pack())
            .expect("painted voxel must exist");

        let mut ref_map = MaterialRefMap::default();
        ref_map.refs.insert(leaf_idx, MaterialRef::Primary);

        // Simulate theme change: re-pack with light palette
        let new_packed = light.voxel_primary.pack();
        if world.nodes[leaf_idx].color_data != new_packed {
            world.nodes[leaf_idx].color_data = new_packed;
            world.mark_dirty(leaf_idx);
        }

        assert_eq!(world.nodes[leaf_idx].color_data, light.voxel_primary.pack());
        assert!(!world.take_dirty_ranges().is_empty());
    }
}
