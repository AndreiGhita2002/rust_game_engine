use std::ops::Range;

use cgmath::Matrix4;
use wgpu::RenderPass;

use crate::{BindGroups, GlobalContext};
use crate::entity::Entity;
use crate::render::instance::{Instance3D, InstanceManager};
use crate::util::SharedCell;

pub mod instance;
pub mod model;
pub mod texture;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    pub _padding: u32,
    pub color: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    pub _padding2: u32,
}

#[allow(dead_code)]
pub struct Renderer {
    label: String,
    render_pipeline: wgpu::RenderPipeline,
    // render functions:
    render_fn_object: Box<dyn RenderFunctions>,
}
impl Renderer {
    pub fn new(
        label: &str,
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        color_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
        vertex_layouts: &[wgpu::VertexBufferLayout],
        shader: wgpu::ShaderModuleDescriptor,
        render_fn_object: Box<dyn RenderFunctions>,
    ) -> Self {
        let shader = device.create_shader_module(shader);

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: vertex_layouts,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
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
            depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
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
        });

        Renderer {
            label: label.to_string(),
            render_pipeline,
            render_fn_object,
        }
    }

    fn init_render_pass<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        instance_manager: &'a InstanceManager,
        bind_groups: &'a BindGroups,
    ) {
        self.render_fn_object.render_init(render_pass, &self, instance_manager, bind_groups)
    }

    fn render_command<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        command: RenderCommand,
        instance_manager: &'a InstanceManager,
        bind_groups: &'a BindGroups,
    ) {
        self.render_fn_object.render(render_pass, command, &self, instance_manager, bind_groups)
    }

    pub fn render<'a>(
        &'a self,
        mut render_pass: RenderPass<'a>,
        commands: Vec<RenderCommand>,
        instance_manager: &'a InstanceManager,
        bind_groups: &'a BindGroups,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        self.init_render_pass(&mut render_pass, instance_manager, bind_groups);
        for command in commands {
            self.render_command(&mut render_pass, command, instance_manager, bind_groups);
        }
    }
}

pub mod preset_renderers {
    use crate::BindGroups;
    use crate::render::{Renderer, StandardRender3d};
    use crate::render::instance::Instance3DRaw;
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
            label: Some("Normal Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../res/shaders/shader.wgsl").into()),
        };

        // render functions:
        let render_fn_object = Box::new(StandardRender3d{});

        Renderer::new(
            "3D Render Pipeline",
            &device,
            &layout,
            config.format,
            Some(Texture::DEPTH_FORMAT),
            &[ModelVertex::desc(), Instance3DRaw::desc()],
            shader,
            render_fn_object,
        )
    }
}


pub trait RenderFunctions {
    fn render_init<'a, 'b>(
        &self,
        render_pass: &mut RenderPass<'b>,
        renderer: &'a Renderer,
        instance_manager: &'a InstanceManager,
        bind_groups: &'a BindGroups,
    ) where 'a: 'b;

    fn render<'a, 'b>(
        &self,
        render_pass: &mut RenderPass<'b>,
        command: RenderCommand,
        renderer: &'a Renderer,
        instance_manager: &'a InstanceManager,
        bind_groups: &'a BindGroups,
    ) where 'a: 'b;
}

pub struct StandardRender3d {}
impl RenderFunctions for StandardRender3d {
    fn render_init<'a, 'b>(
        &self,
        render_pass: &mut RenderPass<'b>,
        _renderer: &'a Renderer,
        instance_manager: &'a InstanceManager,
        bind_groups: &'a BindGroups,
    ) where 'a: 'b {
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
    ) where 'a: 'b {
        let (model_name, _, instances) = command.unpack();
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

pub struct RenderCommand {
    model: String,
    transform: Option<Matrix4<f32>>, //todo: figure out why this there is a matrix in RenderCommand
    instances: Option<Range<u32>>,
}
impl RenderCommand {
    pub fn unpack(self) -> (String, Option<Matrix4<f32>>, Range<u32>) {
        let model = self.model;
        let transform = self.transform;
        let instances = self.instances.unwrap_or(0..1);
        (model, transform, instances)
    }
}

pub trait RenderComponent {
    fn init(&mut self, context: &GlobalContext, entity: &Entity);

    fn render(&self, entity: &Entity, commands: &mut Vec<RenderCommand>);

    fn transform_child(&self, child_command: RenderCommand) -> RenderCommand;

    fn get_name(&self) -> String;
}

pub struct Single3DInstance {
    pub instance_id: u32,
}
impl Single3DInstance {
    pub fn new() -> Box<Self> {
        Box::new(Self { instance_id: 0, })
    }
}
impl RenderComponent for Single3DInstance {
    fn init(&mut self, context: &GlobalContext, entity: &Entity) {
        let id = context.register_instance_3d(entity.instance.unwrap().clone());
        self.instance_id = id as u32;
    }

    fn render(&self, entity: &Entity, commands: &mut Vec<RenderCommand>) {

        commands.push(RenderCommand {
            model: entity.instance.borrow().model_name.clone(),
            transform: None,
            instances: Some(self.instance_id..(self.instance_id+1)),
        })
    }

    fn transform_child(&self, child_command: RenderCommand) -> RenderCommand {
        child_command
    }

    fn get_name(&self) -> String {
        "Single 3D Instance Render".to_string()
    }
}

pub struct NoRender {}
impl NoRender {
    pub fn new() -> Box<Self> {
        Box::new(Self {})
    }
}
impl RenderComponent for NoRender {
    fn init(&mut self, _context: &GlobalContext, _entity: &Entity) {}

    fn render(&self, _entity: &Entity, _commands: &mut Vec<RenderCommand>) {}

    fn transform_child(&self, child_command: RenderCommand) -> RenderCommand { child_command }

    fn get_name(&self) -> String { "No Render".to_string() }
}