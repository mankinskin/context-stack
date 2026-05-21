use super::*;

/// Render graph node that:
/// 1. Dispatches the SVO ray march compute shader.
/// 2. Blits the result to the current `ViewTarget`.
pub struct SvoRayMarchNode {
    view_query: bevy::ecs::query::QueryState<&'static ViewTarget>,
}

impl FromWorld for SvoRayMarchNode {
    fn from_world(world: &mut World) -> Self {
        Self {
            view_query: bevy::ecs::query::QueryState::new(world),
        }
    }
}

impl Node for SvoRayMarchNode {
    fn update(
        &mut self,
        world: &mut World,
    ) {
        self.view_query.update_archetypes(world);
    }

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let Some(compute_pipeline_res) =
            world.get_resource::<SvoRayMarchComputePipeline>()
        else {
            return Ok(());
        };
        let Some(blit_pipeline_res) =
            world.get_resource::<SvoRayMarchBlitPipeline>()
        else {
            return Ok(());
        };
        let Some(compute_bg) =
            world.get_resource::<SvoRayMarchComputeBindGroup>()
        else {
            return Ok(());
        };
        let Some(blit_bg) = world.get_resource::<SvoRayMarchBlitBindGroup>()
        else {
            return Ok(());
        };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>() else {
            return Ok(());
        };
        let Some(rm_buffers) = world.get_resource::<SvoRayMarchBuffers>()
        else {
            return Ok(());
        };

        let Some(compute_pipeline) =
            pipeline_cache.get_compute_pipeline(compute_pipeline_res.0)
        else {
            return Ok(());
        };
        let Some(blit_pipeline) =
            pipeline_cache.get_render_pipeline(blit_pipeline_res.0)
        else {
            return Ok(());
        };

        let binding = self.view_query.query_manual(world);
        let Ok(view_target) = binding.single() else {
            return Ok(());
        };

        let width = rm_buffers.width.max(1);
        let height = rm_buffers.height.max(1);
        let wg_x = (width + 7) / 8;
        let wg_y = (height + 7) / 8;

        let encoder = render_context.command_encoder();

        {
            let mut compute_pass =
                encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("svo_ray_march_compute"),
                    timestamp_writes: None,
                });
            compute_pass.set_pipeline(compute_pipeline);
            compute_pass.set_bind_group(0, &compute_bg.0, &[]);
            compute_pass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        {
            let color_attachment = view_target.get_color_attachment();
            let mut render_pass =
                encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("svo_ray_march_blit"),
                    color_attachments: &[Some(color_attachment)],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            render_pass.set_pipeline(blit_pipeline);
            render_pass.set_bind_group(1, &blit_bg.0, &[]);
            render_pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}
