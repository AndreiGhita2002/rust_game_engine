use std::collections::HashMap;
use std::mem;
use std::ops::Range;

use wgpu::{CommandEncoder, SurfaceTexture};

use crate::entity::component::Component;
use crate::entity::Entity;
use crate::GlobalContext;

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

pub struct RenderCommand {
    pub model: String,
    pub instances: Option<Range<u32>>,
}

impl RenderCommand {
    pub fn unpack(self) -> (String, Range<u32>) {
        let model = self.model;
        let instances = self.instances.unwrap_or(0..1);
        (model, instances)
    }
}

pub struct RenderDispatcher {
    renderers: Vec<Renderer>,
    command_buffer: HashMap<String, Vec<RenderCommand>>,
}
impl RenderDispatcher {
    pub fn new() -> Self {
        Self {
            renderers: Vec::new(),
            command_buffer: HashMap::new(),
        }
    }

    pub fn render(&mut self, context: &GlobalContext) -> Result<(), wgpu::SurfaceError> {
        // output = the new frame that will be drawn on screen
        let mut output = context.surface.get_current_texture()?;
        // dispatching the commands to the renderers
        for renderer in self.renderers.iter() {
            let mut commands =  Vec::new();
            mem::swap(
                &mut commands,
                self.command_buffer.get_mut(&renderer.label).unwrap(),
            );
            renderer.render(context, &mut output, commands);
        }
        // present the output on screen
        output.present();
        Ok(())
    }

    pub fn add_renderer(&mut self, renderer: Renderer) {
        println!("[REN] Renderer added: {}", renderer.label);
        self.command_buffer.insert(renderer.label.clone(), Vec::new());
        self.renderers.push(renderer);
        println!("[REN] Number of renderers: {}", self.renderers.len());
    }

    pub fn push(&mut self, renderer: &str, command: RenderCommand) {
        if let Some(buffer) = self.command_buffer.get_mut(renderer) {
            buffer.push(command)
        }
    }
}

pub struct Renderer {
    label: String,
    render_pipeline: wgpu::RenderPipeline,
    render_fn: Box<dyn RenderFn>,
}
impl Renderer {
    pub fn new(context: &GlobalContext, label: String, render_fn: Box<dyn RenderFn>) -> Self {
        let render_pipeline = render_fn.init_pipeline(context);
        Self { label, render_pipeline, render_fn }
    }

    pub fn render(
        &self,
        context: &GlobalContext,
        output: &mut SurfaceTexture,
        commands: Vec<RenderCommand>
    ) {
        // making the encoder
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        // rendering
        self.render_fn.render(context, output, &mut encoder, &self.render_pipeline, commands);
        // sending the encoded commands away
        context.queue.submit(std::iter::once(encoder.finish()));
    }
}

pub trait RenderFn {
    fn init_pipeline(&self, context: &GlobalContext) -> wgpu::RenderPipeline;

    fn render(
        &self,
        context: &GlobalContext,
        output: &mut SurfaceTexture,
        encoder: &mut CommandEncoder,
        render_pipeline: &wgpu::RenderPipeline,
        commands: Vec<RenderCommand>
    );
}

pub trait RenderComponent {
    fn init(&mut self, context: &GlobalContext, components: &Vec<Component>);

    fn render(&self, entity: &Entity, dispatcher: &mut RenderDispatcher);

    fn get_name(&self) -> String;
}

