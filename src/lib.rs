use std::cell::RefCell;
use std::default::Default;
use std::ops::DerefMut;

use cfg_if::cfg_if;
use cgmath::{Quaternion, Rotation3, Vector3};
use wgpu::Buffer;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowBuilder};

use render::texture::Texture;

use crate::camera::{Camera, CameraUniform, FreeCamController};
use crate::entity::{EntityDesc, EntityManager, EntityRef};
use crate::entity::event::{EventDispatcher, GameEvent};
use crate::entity::render_comp::NoRender;
use crate::entity::space::{GameSpaceMaster, ScreenSpaceMaster};
use crate::entity::system::{PlayerControllerSystem, SystemManager};
use crate::render::{LightUniform, RenderDispatcher, Renderer};
use crate::render::instance::InstanceManager;
use crate::render::render_2d::StandardRender2d;
use crate::render::render_3d::StandardRender3d;
use crate::util::{IdManager, SharedCell};

mod camera;
mod entity;
mod render;
mod resources;
mod util;

pub struct BindGroups {
    pub texture_layout: wgpu::BindGroupLayout,
    pub camera_layout: wgpu::BindGroupLayout,
    pub light_layout: wgpu::BindGroupLayout,
    pub camera: wgpu::BindGroup,
    pub light: wgpu::BindGroup,
}

#[allow(dead_code)]
pub struct GlobalContext {
    // rendering stuff:
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    bind_groups: BindGroups,
    render_dispatcher: RefCell<RenderDispatcher>,
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
    entity_manager: RefCell<EntityManager>,
    system_manager: SharedCell<SystemManager>,
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

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
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
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
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
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let camera_bind_group_layout =
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
                label: Some("camera_bind_group_layout"),
            });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
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
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            // We'll want to update our lights position, so we use COPY_DST
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
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
            light: light_bind_group,
        };

        // managers:
        let id_manager = IdManager::new();
        let event_dispatcher = EventDispatcher::new(id_manager.clone());
        let instance_manager = SharedCell::new(InstanceManager::new(&device, id_manager.clone()));
        let entity_manager = RefCell::new(EntityManager::new(id_manager.clone()));
        let system_manager = SharedCell::new(SystemManager::new(id_manager.clone()));
        let render_dispatcher = RefCell::new(RenderDispatcher::new());

        Self {
            surface,
            device,
            queue,
            config,
            size,
            window,
            bind_groups,
            render_dispatcher,
            camera_buffer,
            depth_texture,
            light_uniform,
            light_buffer,
            id_manager,
            event_dispatcher,
            instance_manager,
            entity_manager,
            system_manager,
            background: [0.0, 0.0, 0.0, 1.0],
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
            // camera aspect:
        }
        self.depth_texture =
            Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
        
        // todo dispatch dynamic event for Screen Resize
        // self.event_dispatcher.send_event(
        //     "window_resize",
        //     GameEvent::ScreenResize {
        //         new_size
        //     }
        // )
        // right now it only get dispatched to all:
        self.input(GameEvent::ScreenResize { new_size })
    }

    pub fn input(&mut self, event: GameEvent) {
        // it's first sent to the systems:
        let _response = self.system_manager.borrow_mut().input(event.clone());
        // if the systems have only weakly used up the event,
        // if response.at_most_weak() {
        //     // then to the event dispatcher to the destination "game_input":
        //     self.event_dispatcher.send_event("game_input", event);
        // }
    }

    pub fn do_tick(&mut self) {
        // dispatching events
        self.event_dispatcher.process_events();

        // systems tick
        self.system_manager.borrow_mut().tick(self);

        // doing tick on the entity graph
        self.entity_manager.borrow_mut().tick();

        // Update the light
        let old_position: Vector3<_> = self.light_uniform.position.into();
        self.light_uniform.position =
            (Quaternion::from_axis_angle((0.0, 1.0, 0.0).into(), cgmath::Deg(1.0)) * old_position)
                .into();
        self.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[self.light_uniform]),
        );

        // instance updates:
        self.instance_manager.borrow_mut().tick(&self);

        // move the cursor to the center of the screen:
        // self.set_cursor_to_center();
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // rendering through the view graph:
        self.entity_manager.borrow().render(self.render_dispatcher.borrow_mut().deref_mut());

        self.render_dispatcher.borrow_mut().render(&self)?;

        Ok(())
    }

    // -----------------------
    //    Utility functions
    // -----------------------
    pub fn update_camera_uniform(&self, camera: &Camera) {
        let uniform = camera.create_uniform();
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    pub async fn async_load_model(&self, model_name: &str) {
        let mut instance_manager = self.instance_manager.borrow_mut();
        if instance_manager.models.contains_key(model_name) {
            return;
        }

        print!("[RES] Loading model {model_name}: ");
        match instance_manager
            .load_model(
                model_name,
                &self.device,
                &self.queue,
                &self.bind_groups.texture_layout,
            ).await
        {
            Ok(()) => println!(" OK"),
            Err(e) => println!(" ERROR: {e}"),
        }
    }

    pub async fn async_load_sprite(&self, sprite_name: &str) {
        let mut instance_manager = self.instance_manager.borrow_mut();
        if instance_manager.models.contains_key(sprite_name) {
            return;
        }

        print!("[RES] Loading sprite {sprite_name}: ");
        match instance_manager
            .load_sprite(
                sprite_name,
                &self.device,
                &self.queue,
                &self.bind_groups.texture_layout,
            ).await
        {
            Ok(()) => println!(" OK"),
            Err(e) => println!(" ERROR: {e}"),
        }
    }

    pub fn load_model(&self, model_name: &str) {
        pollster::block_on(async { self.async_load_model(model_name).await });
    }

    pub fn load_sprite(&self, sprite_name: &str) {
        pollster::block_on(async { self.async_load_sprite(sprite_name).await });
    }

    pub fn set_cursor_to_center(&mut self) {
        if self.window.has_focus() {
            cfg_if! {
                if #[cfg(target_arch = "wasm32")] {
                    //todo figure out how you do mouse look on web
                } else {
                    self.window.set_cursor_position(
                        PhysicalPosition::new(self.size.width / 2, self.size.height / 2)
                    ).unwrap_or_else(|_| println!("Cursor could not be moved!"));
                }
            }
        }
    }
}

fn test_init(context: &mut GlobalContext) {
    // loading models and sprite
    context.load_model("cube");
    context.load_model("cat_cube");
    context.load_sprite("cat");

    // setup the entity manager
    let mut entity_manager = context.entity_manager.borrow_mut();
    {
        // ----- 3D Space -----
        let space_master = entity_manager.new_entity(&context, EntityDesc {
            parent_id: Some(0),
            space_component: Some(Box::new(GameSpaceMaster::default())),
            render_component: Some(NoRender::new()),
            ..Default::default()
        });
        // cubes
        const N: i32 = 5;
        const S: f32 = 2.0;
        for i in -(N / 2)..(N / 2) {
            for j in -(N / 2)..(N / 2) {
                for k in -(N / 2)..(N / 2) {
                    if i == j && j == k && k == 0 {
                        continue
                    }
                    entity_manager.new_entity(&context, EntityDesc {
                        parent_id: Some(space_master.get_id()),
                        position: vec![i as f32 * S, j as f32 * S, k as f32 * S],
                        ..Default::default()
                    });
                }
            }
        }
        // ----- Screen Space -----
        let screen_master = entity_manager.new_entity(&context, EntityDesc {
            parent_id: Some(0),
            space_component: Some(Box::new(ScreenSpaceMaster::default())),
            render_component: Some(NoRender::new()),
            ..Default::default()
        });
        //todo make this show:
        // cat sprite
        entity_manager.new_entity(&context, EntityDesc {
            parent_id: Some(screen_master.get_id()),
            position: vec![0.5, 0.5],
            ..Default::default()
        });
    }
    entity_manager.print_entities();

    // renderers
    let mut render_dispatcher = context.render_dispatcher.borrow_mut();
    // 3d renderer
    render_dispatcher.add_renderer(
        Renderer::new(
            &context,
            "3d".to_string(),
            Box::new(StandardRender3d {}),
        )
    );
    // 2d renderer
    render_dispatcher.add_renderer(
        Renderer::new(
            &context,
            "2d".to_string(),
            Box::new(StandardRender2d {}),
        )
    );

    // player
    let player = entity_manager.new_entity(&context, EntityDesc {
        parent_id: Some(0),
        position: vec![0.0, 0.0, 0.0],
        ..Default::default()
    });

    // systems
    let player_controller = PlayerControllerSystem::new(
        Camera::default(),
        Box::new(FreeCamController::default()),
        player,
    );
    context
        .system_manager
        .borrow_mut()
        .new_system(player_controller);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
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

    // event loop
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == context.window().id() => {
                // view_root_input(&mut view_root, event);
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
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
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::F4),
                                ..
                            },
                        ..
                    } => {
                        context
                            .window
                            .set_fullscreen(Some(Fullscreen::Borderless(None)));
                    }
                    _ => {
                        if let Some(event) = GameEvent::from_window_event(event) {
                            context.input(event)
                        }
                    }
                }
            }
            Event::DeviceEvent { ref event, .. } => {
                if let Some(event) = GameEvent::from_device_event(event) {
                    context.input(event)
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
                context.window().request_redraw();
            }
            _ => {}
        }
    });
}
