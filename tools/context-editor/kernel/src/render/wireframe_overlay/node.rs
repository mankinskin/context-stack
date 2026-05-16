use super::*;

pub struct WireframeOverlayNode {
    view_query: QueryState<&'static ViewTarget>,
}

impl FromWorld for WireframeOverlayNode {
    fn from_world(world: &mut World) -> Self {
        Self {
            view_query: world.query(),
        }
    }
}

impl Node for WireframeOverlayNode {
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
        let Some(pipeline_res) = world.get_resource::<WireframeOverlayPipeline>() else {
            return Ok(());
        };
        let Some(bind_group) = world.get_resource::<WireframeOverlayBindGroup>() else {
            return Ok(());
        };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>() else {
            return Ok(());
        };
        let Some(buffers) = world.get_resource::<WireframeOverlayBuffers>() else {
            return Ok(());
        };
        if buffers.index_count == 0 {
            return Ok(());
        }
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_res.0) else {
            return Ok(());
        };
        let binding = self.view_query.query_manual(world);
        let Ok(view_target) = binding.single() else {
            return Ok(());
        };

        let color_attachment = view_target.get_unsampled_color_attachment();

        // The pipeline has depth_stencil: Some(...), so the render pass MUST
        // provide a depth attachment. Skip the draw when SvoDepthTexture is
        // not yet available (first frame, or headless mode) to avoid a WebGPU
        // validation error.
        let Some(depth_tex) = world
            .get_resource::<crate::render::depth_bridge::SvoDepthTexture>()
        else {
            return Ok(());
        };

        let depth_attachment = RenderPassDepthStencilAttachment {
            view: &depth_tex.view,
            depth_ops: Some(Operations {
                load: LoadOp::Load,
                store: StoreOp::Discard,
            }),
            stencil_ops: None,
        };

        let mut pass = render_context.command_encoder().begin_render_pass(
            &RenderPassDescriptor {
                label: Some("wireframe_overlay_pass"),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: Some(depth_attachment),
                timestamp_writes: None,
                occlusion_query_set: None,
            },
        );
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group.0, &[]);
        pass.set_vertex_buffer(0, *buffers.vertex_buf.slice(..));
        pass.set_index_buffer(*buffers.index_buf.slice(..), IndexFormat::Uint32);
        pass.draw_indexed(0..buffers.index_count, 0, 0..1);

        Ok(())
    }
}
