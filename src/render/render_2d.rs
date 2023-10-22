use wgpu::{CommandEncoder, RenderPass, RenderPassDescriptor, TextureView};

use crate::{BindGroups, GlobalContext};
use crate::entity::component::Component;
use crate::entity::Entity;
use crate::render::{RenderCommand, RenderComponent, Renderer, RenderFunctions};
use crate::render::instance::{Instance2DRaw, InstanceManager, InstanceRef};
use crate::render::model::{SpriteVertex, Vertex};
use crate::render::texture::Texture;

pub fn preset_renderer_2d(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    bind_groups: &BindGroups,
) -> Renderer {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("2D Render Pipeline Layout"),
        // todo if changing bind groups is too intensive
        //  then make this common with 3d renderer
        bind_group_layouts: &[
            &bind_groups.texture_layout,
            &bind_groups.camera_layout,
        ],
        push_constant_ranges: &[],
    });
    let shader = wgpu::ShaderModuleDescriptor {
        label: Some("2D Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../res/shaders/sprite.wgsl").into()),
    };

    // render functions:
    let render_fn_object = Box::new(StandardRender2d {});

    Renderer::new(
        "2d",
        &device,
        &layout,
        config.format,
        Some(Texture::DEPTH_FORMAT),
        &[SpriteVertex::desc(), Instance2DRaw::desc()],
        shader,
        render_fn_object,
    )
}

pub struct StandardRender2d {}
impl RenderFunctions for StandardRender2d {
    fn begin_render_pass<'a>(
        &'a self,
        encoder: &'a mut CommandEncoder,
        context: &'a GlobalContext,
        texture_view: &'a TextureView
    ) -> RenderPass<'a> {
        encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("2D Render Pass"),
            color_attachments: &[
                // This is what @location(0) in the fragment shader targets
                Some(wgpu::RenderPassColorAttachment {
                    view: texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
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
        render_pass.set_vertex_buffer(1, instance_manager.instance_2d_buffer.slice(..));
        render_pass.set_bind_group(1, &bind_groups.camera, &[]);
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
        //this is the same as the 3d one
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
            println!("[RENDER] Sprite not found: {}", model_name)
        }
    }
}

pub struct SingleSpriteComponent {
    pub sprite_name: String,
    pub instance_ref: InstanceRef,
}

impl SingleSpriteComponent {
    pub fn new(sprite_name: &str, instance_ref: InstanceRef) -> Box<Self> {
        Box::new(Self {
            instance_ref,
            sprite_name: sprite_name.to_string(),
        })
    }
}

impl RenderComponent for SingleSpriteComponent {
    fn init(&mut self, _context: &GlobalContext, _components: &Vec<Component>) {}

    fn render(&self, _entity: &Entity, commands: &mut Vec<RenderCommand>) {
        let i = self.instance_ref.get_instance_id();
        commands.push(RenderCommand {
            renderer: "2d".to_string(),
            model: self.sprite_name.clone(),
            transform: None,
            instances: Some(i..(i + 1)),
        })
    }

    fn transform_child(&self, child_command: RenderCommand) -> RenderCommand {
        child_command
    }

    fn get_name(&self) -> String {
        "Single 2D Sprite Render".to_string()
    }
}
