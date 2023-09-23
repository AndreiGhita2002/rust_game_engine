use std::ops::Deref;

use cgmath::Rotation3;
use wgpu::Buffer;
use wgpu::util::DeviceExt;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowBuilder};

use render::texture::Texture;

use crate::camera::CameraUniform;
use crate::entity::{Entity, EntityManager};
use crate::event::{EventDispatcher, GameEvent};
use crate::render::{LightUniform, Renderer};
use crate::render::instance::{Instance3D, InstanceManager};
use crate::util::{IdManager, SharedCell};

mod util;
mod entity;
mod render;
mod camera;
mod event;
mod resources;

pub struct BindGroups {
    pub texture_layout: wgpu::BindGroupLayout,
    pub camera_layout: wgpu::BindGroupLayout,
    pub light_layout: wgpu::BindGroupLayout,
    pub camera: wgpu::BindGroup,
    pub light: wgpu::BindGroup,
}

pub struct GlobalContext {
    // rendering stuff:
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    bind_groups: BindGroups,
    renderer_3d: Renderer,
    // camera stuff:
    camera_buffer: Buffer,
    // depth texture:
    depth_texture: Texture,
    // lighting:
    light_uniform: LightUniform,
    light_buffer: Buffer,
    // game managers:
    id_manager: IdManager,
    event_dispatcher: EventDispatcher,
    instance_manager: SharedCell<InstanceManager>,
    entity_manager: SharedCell<EntityManager>,
    // background colour:
    background: [f64; 4],
}
impl GlobalContext {
    pub async fn new(window: Window) -> Self {
        let size = window.inner_size();
        window.set_cursor_visible(false);
        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });
        // # Safety
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                // WebGL doesn't support all of wgpu's features, so if
                // we're building for the web we'll have to disable some.
                limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                label: None,
            },
            None, // Trace path
        ).await.unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps.formats.iter()
            .copied()
            .filter(|f| f.is_srgb())
            .next()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // image stuff:
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    // This should match the filterable field of the
                    // corresponding Texture entry above.
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });

        // camera:
        // let camera = Camera::default().with_aspect(config.width as f32 / config.height as f32);
        let camera_uniform = CameraUniform::new();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: Some("camera_bind_group_layout"),
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }
            ],
            label: Some("camera_bind_group"),
        });

        // depth texture:
        let depth_texture = Texture::create_depth_texture(&device, &config, "depth_texture");

        // light uniform:
        let light_uniform = LightUniform {
            position: [2.0, 2.0, 2.0],
            _padding: 0,
            color: [1.0, 1.0, 1.0],
            _padding2: 0,
        };
        let light_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Light VB"),
                contents: bytemuck::cast_slice(&[light_uniform]),
                // We'll want to update our lights position, so we use COPY_DST
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: None,
            });
        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
            label: None,
        });

        let bind_groups = BindGroups {
            camera_layout: camera_bind_group_layout,
            texture_layout: texture_bind_group_layout,
            light_layout: light_bind_group_layout,
            camera: camera_bind_group,
            light: light_bind_group
        };

        // managers:
        let id_manager = IdManager::new();
        let event_dispatcher = EventDispatcher::new(id_manager.clone());
        let instance_manager = SharedCell::new(InstanceManager::new(&device, id_manager.clone()));
        let entity_manager = SharedCell::new(EntityManager::new(id_manager.clone()));

        // renderers:
        let renderer_3d = render::preset_renderers::preset_renderer_3d(
            &device,
            &config,
            &bind_groups,
        );

        Self {
            surface, device, queue, config, size, window,
            bind_groups,
            renderer_3d,
            camera_buffer,
            depth_texture,
            light_uniform,
            light_buffer,
            id_manager,
            event_dispatcher,
            instance_manager,
            entity_manager,
            background: [0.1, 0.1, 0.1, 1.0],
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
        self.depth_texture = Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
    }

    pub fn input(&mut self, event: GameEvent) {
        let _ = event;
        //todo
    }

    pub fn do_tick(&mut self) {
        // dispatching events
        self.event_dispatcher.process_events();

        // doing tick on the entity graph
        self.entity_manager.borrow_mut().tick();

        // Update the light
        let old_position: cgmath::Vector3<_> = self.light_uniform.position.into();
        self.light_uniform.position =
            (cgmath::Quaternion::from_axis_angle((0.0, 1.0, 0.0).into(), cgmath::Deg(1.0))
                * old_position)
                .into();
        self.queue.write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(&[self.light_uniform]));

        // instance updates:
        self.instance_manager.borrow_mut().tick(&self);

        // move the cursor to the center of the screen:
        // self.set_cursor_to_center();
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let instance_manager = self.instance_manager.borrow();
        let output = self.surface.get_current_texture()?;
        let texture_view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        // rendering through the view graph:
        println!("Number of Entities: {}", self.entity_manager.borrow().len());
        let commands = self.entity_manager.borrow().render();
        println!("Render Commands: {}", commands.len());

        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[
                    // This is what @location(0) in the fragment shader targets
                    Some(wgpu::RenderPassColorAttachment {
                        view: &texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(
                                wgpu::Color {
                                    r: self.background[0],
                                    g: self.background[1],
                                    b: self.background[2],
                                    a: self.background[3],
                                }
                            ),
                            store: true,
                        }
                    })
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
            self.renderer_3d.render(render_pass, commands, instance_manager.deref(), &self.bind_groups);
        }
        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    // -----------------------
    //    Utility functions
    // -----------------------
    pub fn register_entity(&self, entity: SharedCell<Entity>) {
        let mut entity_manager = self.entity_manager.borrow_mut();
        entity_manager.register_entity(entity)
    }

    pub fn register_instance_3d(&self, instance_3d: SharedCell<Instance3D>) -> usize {
        let mut instance_manager = self.instance_manager.borrow_mut();
        instance_manager.register_instance(instance_3d)
    }

    pub async fn async_load_model(&self, file_name: &str) {
        let mut instance_manager = self.instance_manager.borrow_mut();
        if instance_manager.models.contains_key(file_name) {
            return;
        }

        print!("[RES] Loading model {file_name}: ");
        match instance_manager.load_model(
            file_name,
            &self.device,
            &self.queue,
            &self.bind_groups.texture_layout
        ).await {
            Ok(_) => {println!(" OK")}
            Err(e) => {println!(" ERROR: {e}")}
        }
    }

    pub fn load_model(&self, file_name: &str) {
        pollster::block_on(async {
            self.async_load_model(file_name).await
        });
    }
}

fn test_init(context: &mut GlobalContext) {
    context.load_model("cube");
    context.load_model("cat_cube");

    let mut entity_manager = context.entity_manager.borrow_mut();
    entity_manager.init(&context);
}

#[cfg_attr(target_arch="wasm32", wasm_bindgen(start))]
pub async fn run() {
    // start the logger
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
        } else {
            env_logger::init();
        }
    }
    // window setup
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        // Winit prevents sizing with CSS, so we have to set
        // the size manually when on web.
        use winit::dpi::PhysicalSize;
        window.set_inner_size(PhysicalSize::new(1000, 1000));

        // from the tutorial:
        //  "The "wasm-example" id is specific to my project (aka. this tutorial).
        //  You can substitute this for whatever id you're using in your HTML.
        //  Alternatively, you could add the canvas directly to the <body> as
        //  they do in the wgpu repo. This part is ultimately up to you."

        use winit::platform::web::WindowExtWebSys;
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| {
                let dst = doc.get_element_by_id("wasm-example")?;
                let canvas = web_sys::Element::from(window.canvas());
                dst.append_child(&canvas).ok()?;
                Some(())
            })
            .expect("Couldn't append canvas to document body.");
        println!("hello web!")
    }

    // initialising the global state
    let mut context = GlobalContext::new(window).await;
    test_init(&mut context);
    context.do_tick();
    let mut redraw = false;

    // event loop
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent { ref event, window_id, }
            if window_id == context.window().id() => {
                // view_root_input(&mut view_root, event);
                match event {
                    WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => {
                        context.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        context.resize(**new_inner_size);
                    }
                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Space),
                            ..
                        },
                        ..
                    } => {
                        redraw = true;
                    }
                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::F4),
                            ..
                        },
                        ..
                    } => {
                        context.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    }
                    _ => if let Some(event) = GameEvent::from_winit_event(event) {
                        context.input(event)
                    }
                }
            }
            Event::RedrawRequested(window_id) if window_id == context.window().id() => {
                context.do_tick();
                match context.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => context.resize(context.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                if redraw {
                    println!("REDRAW!!");
                    context.window().request_redraw();
                    redraw = false;
                }
            }
            _ => {}
        }
    });
}
