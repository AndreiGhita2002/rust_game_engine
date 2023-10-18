use std::ops::Range;

use cgmath::Matrix4;
use wgpu::RenderPass;

use crate::{BindGroups, GlobalContext};
use crate::entity::component::Component;
use crate::entity::Entity;
use crate::render::instance::InstanceManager;

pub mod instance;
pub mod model;
pub mod texture;
pub mod render_3d;
pub mod render_2d;

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
        self.render_fn_object
            .render_init(render_pass, &self, instance_manager, bind_groups)
    }

    fn render_command<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        command: RenderCommand,
        instance_manager: &'a InstanceManager,
        bind_groups: &'a BindGroups,
    ) {
        self.render_fn_object
            .render(render_pass, command, &self, instance_manager, bind_groups)
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        commands: &mut Vec<RenderCommand>,
        instance_manager: &'a InstanceManager,
        bind_groups: &'a BindGroups,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        self.init_render_pass(render_pass, instance_manager, bind_groups);

        // iterates through commands and take ownership of the ones that match the current renderer
        let mut l = commands.len();
        let mut i = 0;
        while i < l {
            if commands[i].renderer == self.label {
                let command = commands.remove(i);
                self.render_command(render_pass, command, instance_manager, bind_groups);
                l -= 1;
            } else {
                i += 1;
            }

        }
    }
}

pub trait RenderFunctions {
    fn render_init<'a, 'b>(
        &self,
        render_pass: &mut RenderPass<'b>,
        renderer: &'a Renderer,
        instance_manager: &'a InstanceManager,
        bind_groups: &'a BindGroups,
    ) where
        'a: 'b;

    fn render<'a, 'b>(
        &self,
        render_pass: &mut RenderPass<'b>,
        command: RenderCommand,
        renderer: &'a Renderer,
        instance_manager: &'a InstanceManager,
        bind_groups: &'a BindGroups,
    ) where
        'a: 'b;
}

pub struct RenderCommand {
    pub renderer: String,
    pub model: String,
    pub transform: Option<Matrix4<f32>>, //todo: figure out what to do with this and implement it
    pub instances: Option<Range<u32>>,
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
    fn init(&mut self, context: &GlobalContext, components: &Vec<Component>);

    fn render(&self, entity: &Entity, commands: &mut Vec<RenderCommand>);

    // todo is this necessary:
    fn transform_child(&self, child_command: RenderCommand) -> RenderCommand;

    fn get_name(&self) -> String;
}

pub struct NoRender {}
impl NoRender {
    pub fn new() -> Box<Self> {
        Box::new(Self {})
    }
}
impl RenderComponent for NoRender {
    fn init(&mut self, _context: &GlobalContext, _components: &Vec<Component>) {}

    fn render(&self, _entity: &Entity, _commands: &mut Vec<RenderCommand>) {}

    fn transform_child(&self, child_command: RenderCommand) -> RenderCommand {
        child_command
    }

    fn get_name(&self) -> String {
        "No Render".to_string()
    }
}
