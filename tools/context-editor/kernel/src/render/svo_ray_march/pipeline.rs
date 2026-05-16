use super::*;

/// Compute bind group layout matching `svo_ray_march.wgsl` group(0).
fn compute_bind_group_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_svo_ray_march_compute",
        &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    )
}

/// Empty placeholder layout for group 0 in the blit pipeline.
fn blit_dummy_group0_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new("bgl_svo_blit_group0_empty", &[])
}

/// Blit bind group layout matching `svo_ray_march.wgsl` group(1).
fn blit_bind_group_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_svo_ray_march_blit",
        &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    )
}

/// Cached compute pipeline for the ray march dispatch.
#[derive(Resource)]
pub struct SvoRayMarchComputePipeline(pub CachedComputePipelineId);

/// Cached render pipeline for the blit pass.
#[derive(Resource)]
pub struct SvoRayMarchBlitPipeline(pub CachedRenderPipelineId);

/// Pre-built compute bind group (rebuilt each frame because buffers can resize).
#[derive(Resource)]
pub struct SvoRayMarchComputeBindGroup(pub BindGroup);

/// Pre-built blit bind group (rebuilt each frame because buffers can resize).
#[derive(Resource)]
pub struct SvoRayMarchBlitBindGroup(pub BindGroup);

/// Queue both pipelines for compilation (runs once at startup).
pub fn queue_ray_march_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing_cmp: Option<Res<SvoRayMarchComputePipeline>>,
    existing_blit: Option<Res<SvoRayMarchBlitPipeline>>,
) {
    let shader = asset_server
        .load("embedded://context_editor_kernel/render/svo_ray_march.wgsl");

    if existing_cmp.is_none() {
        let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("svo_ray_march_compute".into()),
            layout: vec![compute_bind_group_layout()],
            push_constant_ranges: vec![],
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Some("ray_march_main".into()),
            zero_initialize_workgroup_memory: true,
        });
        commands.insert_resource(SvoRayMarchComputePipeline(id));
    }

    if existing_blit.is_none() {
        let id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("svo_ray_march_blit".into()),
            layout: vec![blit_dummy_group0_layout(), blit_bind_group_layout()],
            push_constant_ranges: vec![],
            vertex: VertexState {
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Some("blit_vs".into()),
                buffers: vec![],
            },
            fragment: Some(FragmentState {
                shader,
                shader_defs: vec![],
                entry_point: Some("blit_fs".into()),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::bevy_default(),
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::Zero,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent::OVER,
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState {
                count: 4,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            zero_initialize_workgroup_memory: false,
        });
        commands.insert_resource(SvoRayMarchBlitPipeline(id));
    }
}

/// Rebuild both bind groups each frame (buffers may be recreated on resize).
pub fn rebuild_ray_march_bind_groups(
    mut commands: Commands,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    rm_buffers: Option<Res<SvoRayMarchBuffers>>,
    rm_uniforms: Option<Res<SvoRayMarchUniformBuffer>>,
    svo: Option<Res<SvoDoubleBuffer>>,
    svo_tf: Option<Res<SvoTransformBuffer>>,
    page_table_buf: Option<Res<SvoPageTableBuffer>>,
) {
    let Some(rm_buffers) = rm_buffers else { return };
    let Some(rm_uniforms) = rm_uniforms else { return };
    let Some(svo) = svo else { return };
    let Some(svo_tf) = svo_tf else { return };

    let page_table_binding = page_table_buf
        .as_ref()
        .map(|page_table| page_table.buffer.as_entire_binding())
        .unwrap_or_else(|| svo.read_source().as_entire_binding());

    {
        let layout = pipeline_cache.get_bind_group_layout(&compute_bind_group_layout());
        let bind_group = device.create_bind_group(
            "bg_svo_ray_march_compute",
            &layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: svo.read_source().as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: rm_uniforms.0.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: svo_tf.0.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: rm_buffers.depth.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: rm_buffers.color.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: page_table_binding,
                },
            ],
        );
        commands.insert_resource(SvoRayMarchComputeBindGroup(bind_group));
    }

    {
        let layout = pipeline_cache.get_bind_group_layout(&blit_bind_group_layout());
        let bind_group = device.create_bind_group(
            "bg_svo_ray_march_blit",
            &layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: rm_buffers.color.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: rm_uniforms.0.as_entire_binding(),
                },
            ],
        );
        commands.insert_resource(SvoRayMarchBlitBindGroup(bind_group));
    }
}
