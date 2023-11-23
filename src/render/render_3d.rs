use wgpu::{CommandEncoder, RenderPassDescriptor, RenderPipeline, SurfaceTexture};

use crate::entity::component::Component;
use crate::entity::Entity;
use crate::GlobalContext;
use crate::render::{RenderCommand, RenderComponent, RenderDispatcher, RenderFn};
use crate::render::instance::{Instance3DRaw, InstanceRef};
use crate::render::model::{ModelVertex, Vertex};
use crate::render::texture::Texture;

pub struct StandardRender3d {}
impl RenderFn for StandardRender3d {
    fn init_pipeline(&self, context: &GlobalContext) -> RenderPipeline {
        let layout = context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("3D Render Pipeline Layout"),
            bind_group_layouts: &[
                &context.bind_groups.texture_layout,
                &context.bind_groups.camera_layout,
                &context.bind_groups.light_layout,
            ],
            push_constant_ranges: &[],
        });
        let shader = context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("3D Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../res/shaders/shader.wgsl").into()),
        });
        context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("3d pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[ModelVertex::desc(), Instance3DRaw::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: context.config.format,
                    blend: Some(wgpu::BlendState {
                        alpha: wgpu::BlendComponent::REPLACE,
                        color: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: Some(Texture::DEPTH_FORMAT).map(|format| wgpu::DepthStencilState {
                format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
    }

    fn render(&self,
              context: &GlobalContext,
              output: &mut SurfaceTexture,
              encoder: &mut CommandEncoder,
              render_pipeline: &RenderPipeline,
              mut commands: Vec<RenderCommand>,
    ) {
        //this is the same as the 2d one
        let instance_manager = context.instance_manager.borrow();
        let texture_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("3D Render Pass"),
            color_attachments: &[
                // This is what @location(0) in the fragment shader targets
                Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: context.background[0],
                            g: context.background[1],
                            b: context.background[2],
                            a: context.background[3],
                        }),
                        store: true,
                    },
                }),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &context.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        render_pass.set_pipeline(render_pipeline);
        render_pass.set_vertex_buffer(1, instance_manager.instance_3d_buffer.slice(..));
        render_pass.set_bind_group(1, &context.bind_groups.camera, &[]);
        render_pass.set_bind_group(2, &context.bind_groups.light, &[]);

        for command in commands.into_iter() {
            let (model_name, instances) = command.unpack();
            if let Some(model) = instance_manager.models.get(&model_name) {
                for mesh in &model.meshes {
                    let material = &model.materials[mesh.material];
                    render_pass.set_bind_group(0, &material.bind_group, &[]);
                    render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..mesh.num_elements, 0, instances.clone());
                }
            } else {
                println!("[RENDER] Model not found: {}", model_name)
            }
        }
    }
}

pub struct SingleModelComponent {
    pub model_name: String,
    pub instance_ref: InstanceRef,
}

impl SingleModelComponent {
    pub fn new(model_name: &str, instance_ref: InstanceRef) -> Box<Self> {
        Box::new(Self {
            instance_ref,
            model_name: model_name.to_string(),
        })
    }
}

impl RenderComponent for SingleModelComponent {
    fn init(&mut self, _context: &GlobalContext, _components: &Vec<Component>) {}

    fn render(&self, _entity: &Entity, dispatcher: &mut RenderDispatcher) {
        let i = self.instance_ref.get_instance_id();
        dispatcher.push(
            "3d",
            RenderCommand {
                model: self.model_name.clone(),
                instances: Some(i..(i + 1)),
            },
        )
    }

    fn get_name(&self) -> String {
        "Single 3D Model Render".to_string()
    }
}
