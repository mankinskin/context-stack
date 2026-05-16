use super::*;

/// Render-world bind groups for the front/back SVO buffers.
///
/// The active bind group matches the buffer currently read by the GPU, while
/// the other bind group points at the buffer being written by the CPU.
///
/// [`active_svo_group`]: DoubleBindGroups::active_svo_group
#[derive(Resource)]
pub struct DoubleBindGroups {
    front_svo_group: BindGroup,
    back_svo_group: BindGroup,
}

impl DoubleBindGroups {
    /// Build both bind groups from the pre-allocated buffers.
    pub fn new(
        device: &RenderDevice,
        layout: &GpuBindGroupLayouts,
        svo: &SvoDoubleBuffer,
        camera: &CameraUniformBuffer,
        globals: &GlobalUniformBuffer,
    ) -> Self {
        let make_group = |svo_buf: &Buffer| {
            // Bevy 0.18 API: create_bind_group(label, layout, entries)
            device.create_bind_group(
                "bg_svo",
                &layout.svo_group,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: svo_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: camera.0.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: globals.0.as_entire_binding(),
                    },
                ],
            )
        };
        Self {
            front_svo_group: make_group(&svo.front),
            back_svo_group: make_group(&svo.back),
        }
    }

    /// Returns the bind group for the SVO buffer currently being read by the GPU.
    pub fn active_svo_group(
        &self,
        current_is_front: bool,
    ) -> &BindGroup {
        if current_is_front {
            &self.front_svo_group
        } else {
            &self.back_svo_group
        }
    }
}
