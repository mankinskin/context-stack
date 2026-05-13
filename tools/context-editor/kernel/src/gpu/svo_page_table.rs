//! SVO page table GPU buffer — maps virtual page IDs to physical buffer offsets.
//!
//! Phase 4a: the ray march shader reads this buffer to translate leaf-page
//! virtual IDs (stored in page-boundary node `first_child_idx` fields) to
//! physical node indices in the packed GPU octree buffer.
//!
//! A value of `0xFFFFFFFF` indicates a non-resident (evicted) page; the shader
//! treats a non-resident page hit as an opaque solid using the nearest
//! resident ancestor's propagated average color.

use bevy::{
    prelude::Resource,
    render::{
        extract_resource::ExtractResource,
        render_resource::{
            Buffer,
            BufferDescriptor,
            BufferUsages,
        },
        renderer::RenderDevice,
    },
};

// ---------------------------------------------------------------------------
// SvoPageTableBuffer
// ---------------------------------------------------------------------------

/// GPU storage buffer holding the page table (`array<u32>`).
///
/// Size is rebuilt whenever the total number of leaf pages changes.  For a
/// depth-8 SVO with `page_depth=4`, the maximum number of pages is 4096
/// (8^4 / 8 × 8 ≈ 4096 roots at depth 4), so the initial allocation of 4096
/// u32s (16 KiB) is correct for the default configuration and rarely needs to
/// grow.
#[derive(Resource, Clone)]
pub struct SvoPageTableBuffer {
    pub buffer: Buffer,
    /// Number of u32 entries currently allocated.
    pub capacity: usize,
}

impl ExtractResource for SvoPageTableBuffer {
    type Source = SvoPageTableBuffer;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

impl SvoPageTableBuffer {
    /// Minimum allocation: 4096 pages × 4 bytes = 16 KiB.
    const MIN_PAGES: usize = 4096;

    pub fn new(
        device: &RenderDevice,
        page_count: usize,
    ) -> Self {
        let capacity = page_count.max(Self::MIN_PAGES);
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("svo_page_table"),
            size: (capacity * 4) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { buffer, capacity }
    }
}
