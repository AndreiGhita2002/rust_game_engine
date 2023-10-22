use wgpu::{CommandEncoder, RenderPass, RenderPassDescriptor, TextureView};

use crate::{BindGroups, GlobalContext};
use crate::entity::component::Component;
use crate::entity::Entity;
use crate::render::{RenderCommand, RenderComponent, Renderer, RenderFunctions};
use crate::render::instance::{Instance3DRaw, InstanceManager, InstanceRef};
use crate::render::model::{ModelVertex, Vertex};
use crate::render::texture::Texture;

pub fn preset_renderer_3d(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    bind_groups: &BindGroups,
) -> Renderer {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("3D Render Pipeline Layout"),
        bind_group_layouts: &[
            &bind_groups.texture_layout,
            &bind_groups.camera_layout,
            &bind_groups.light_layout,
        ],
        push_constant_ranges: &[],
    });
    let shader = wgpu::ShaderModuleDescriptor {
        label: Some("3D Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../res/shaders/shader.wgsl").into()),
    };

    // render functions:
    let render_fn_object = Box::new(StandardRender3d {});

    Renderer::new(
        "3d",
        &device,
        &layout,
        config.format,
        Some(Texture::DEPTH_FORMAT),
        &[ModelVertex::desc(), Instance3DRaw::desc()],
        shader,
        render_fn_object,
    )
}

pub struct StandardRender3d {}
impl RenderFunctions for StandardRender3d {
    fn begin_render_pass<'a>(
        &'a self,
        encoder: &'a mut CommandEncoder,
        context: &'a GlobalContext,
        texture_view: &'a TextureView
    ) -> RenderPass<'a> {
        encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("3D Render Pass"),
            color_attachments: &[
                // This is what @location(0) in the fragment shader targets
                Some(wgpu::RenderPassColorAttachment {
                    view: texture_view,
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
        })
    }

    fn render_init<'a, 'b>(
        &self,
        render_pass: &mut RenderPass<'b>,
        _renderer: &'a Renderer,
        instance_manager: &'a InstanceManager,
        bind_groups: &'a BindGroups,
    ) where
        'a: 'b,
    {
        render_pass.set_vertex_buffer(1, instance_manager.instance_3d_buffer.slice(..));
        render_pass.set_bind_group(1, &bind_groups.camera, &[]);
        render_pass.set_bind_group(2, &bind_groups.light, &[]);
    }

    fn render<'a, 'b>(
        &self,
        render_pass: &mut RenderPass<'b>,
        command: RenderCommand,
        _renderer: &'a Renderer,
        instance_manager: &'a InstanceManager,
        _bind_groups: &'a BindGroups,
    ) where
        'a: 'b,
    {
        // todo use the transform:
        let (model_name, _transform, instances) = command.unpack();
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

    fn render(&self, _entity: &Entity, commands: &mut Vec<RenderCommand>) {
        let i = self.instance_ref.get_instance_id();
        commands.push(RenderCommand {
            renderer: "3d".to_string(),
            model: self.model_name.clone(),
            transform: None,
            instances: Some(i..(i + 1)),
        })
    }

    fn transform_child(&self, child_command: RenderCommand) -> RenderCommand {
        child_command
    }

    fn get_name(&self) -> String {
        "Single 3D Model Render".to_string()
    }
}
