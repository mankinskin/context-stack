use bytemuck::{
    Pod,
    Zeroable,
};

/// A single node in the Sparse Voxel Octree.
///
/// Stored in a flat `Vec<OctreeNode>` that is uploaded verbatim to the GPU.
/// 8 bytes per node (2 × u32) for cache-line efficiency.
///
/// WGSL struct (must stay in sync):
/// ```wgsl
/// struct OctreeNode {
///     child_pointer: u32,  // lower 8 bits = child bitmask; upper 24 = first-child slot index
///     color_data:    u32,  // R8 G8 B8 + roughness5 + metallic1 + reserved2
/// }
/// ```
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct OctreeNode {
    /// Lower 8 bits: bitmask of which of the 8 children exist.
    /// Upper 24 bits: index (into `VoxelWorld::nodes`) of the first child slot.
    pub child_pointer: u32,
    /// Packed material data. For leaf nodes: R8G8B8 color + roughness/metallic flags.
    pub color_data: u32,
}

impl OctreeNode {
    /// Bit 31 of `child_pointer`: marks a leaf node as fully surrounded on all 6 faces.
    /// Interior voxels are skipped by the splat kernel — they can never be seen.
    pub const INTERIOR_FLAG: u32 = 0x8000_0000;

    pub const fn leaf(color_data: u32) -> Self {
        Self {
            child_pointer: 0,
            color_data,
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.child_mask() == 0
    }

    pub fn child_mask(&self) -> u8 {
        (self.child_pointer & 0xFF) as u8
    }

    pub fn first_child_index(&self) -> usize {
        ((self.child_pointer & 0x7FFF_FF00) >> 8) as usize
    }
}

/// Material packed into an `OctreeNode::color_data` u32.
///
/// Bit layout:
/// ```text
///   Bits  0–7:  R (8 bits)
///   Bits  8–15: G (8 bits)
///   Bits 16–23: B (8 bits)
///   Bits 24–28: Roughness (5 bits, 0–31 → 0.0–1.0)
///   Bit  29:    Metallic  (1 bit, 0 = dielectric, 1 = metallic)
///   Bits 30–31: SDF type  (0=box, 1=sphere, 2=svo-sampled, 3=torus/procedural)
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct VoxelMaterial {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Perceptual roughness quantised to 5 bits (0–31).
    /// Values above 31 are clamped.
    pub roughness: u8,
    /// Metallic flag (binary: dielectric or metal).
    pub metallic: bool,
    /// SDF shape type encoded in bits 30–31:
    /// 0 = box (default), 1 = sphere, 2 = svo-sampled, 3 = torus.
    pub sdf_type: u8,
}

impl VoxelMaterial {
    pub const fn new(
        r: u8,
        g: u8,
        b: u8,
        roughness: u8,
    ) -> Self {
        Self {
            r,
            g,
            b,
            roughness,
            metallic: false,
            sdf_type: 0,
        }
    }

    pub const fn new_metallic(
        r: u8,
        g: u8,
        b: u8,
        roughness: u8,
        metallic: bool,
    ) -> Self {
        Self {
            r,
            g,
            b,
            roughness,
            metallic,
            sdf_type: 0,
        }
    }

    pub const fn new_sdf(
        r: u8,
        g: u8,
        b: u8,
        roughness: u8,
        sdf_type: u8,
    ) -> Self {
        Self {
            r,
            g,
            b,
            roughness,
            metallic: false,
            sdf_type,
        }
    }

    pub const fn pack(self) -> u32 {
        let roughness = self.roughness;
        let rough_5 = (if roughness < 31 { roughness } else { 31 }) as u32;
        let metal_bit = if self.metallic { 1u32 } else { 0u32 };
        let sdf_bits = (self.sdf_type as u32 & 3u32) << 30;
        (self.r as u32)
            | (self.g as u32) << 8
            | (self.b as u32) << 16
            | rough_5 << 24
            | metal_bit << 29
            | sdf_bits
    }

    pub fn unpack(v: u32) -> Self {
        Self {
            r: v as u8,
            g: (v >> 8) as u8,
            b: (v >> 16) as u8,
            roughness: ((v >> 24) & 0x1F) as u8,
            metallic: ((v >> 29) & 1) != 0,
            sdf_type: ((v >> 30) & 3) as u8,
        }
    }
}
