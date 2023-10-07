use std::cell::Cell;
use std::fmt;
use std::fmt::Formatter;

use cgmath::{InnerSpace, Matrix4, Point3, SquareMatrix, Vector3};
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyboardInput, VirtualKeyCode};

use crate::event::GameEvent;

#[rustfmt::skip]
#[allow(dead_code)]
pub const OPENGL_TO_WGPU_MATRIX: Matrix4<f32> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

pub struct Camera {
    pub eye: Point3<f32>,
    pub target: Point3<f32>,
    pub up: Vector3<f32>,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    fn build_view_projection_matrix(&self) -> Matrix4<f32> {
        //The view matrix moves the world to be at the position and rotation of the camera.
        //It's essentially an inverse of whatever the transform matrix of the camera would be.
        let view = Matrix4::look_at_rh(self.eye, self.target, self.up);
        //The proj matrix warps the scene to give the effect of depth.
        // Without this, objects up close would be the same size as objects far away.
        let proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
        //The coordinate system in Wgpu is based on DirectX, and Metal's coordinate systems.
        // That means that in normalized device coordinates (opens new window) the x axis and y axis
        // are in the range of -1.0 to +1.0, and the z axis is 0.0 to +1.0. The cgmath crate
        // (as well as most game math crates) is built for OpenGL's coordinate system.
        // This matrix will scale and translate our scene from OpenGL's coordinate system to WGPU's.
        return OPENGL_TO_WGPU_MATRIX * proj * view;
    }

    pub fn with_aspect(mut self, aspect: f32) -> Self {
        self.aspect = aspect;
        self
    }

    pub fn get_pos(&self) -> Point3<f32> {
        self.eye
    }

    pub fn set_pos(&mut self, eye_pos: (f32, f32, f32)) {
        self.eye = Point3::from(eye_pos);
    }

    pub fn set_target_pos(&mut self, target_pos: (f32, f32, f32)) {
        self.target = Point3::from(target_pos);
    }

    pub fn create_uniform(&self) -> CameraUniform {
        CameraUniform {
            view_position: self.eye.to_homogeneous().into(),
            view_proj: (OPENGL_TO_WGPU_MATRIX * self.build_view_projection_matrix()).into(),
        }
    }

    #[allow(dead_code)]
    pub fn print(&self) {
        println!("camera( eye: {:?}, target: {:?} )", self.eye, self.target)
    }
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            eye: (0.0, 0.0, 0.0).into(),
            target: (1.0, 2.0, 0.0).into(),
            up: Vector3::unit_y(),
            aspect: 1.0,
            fovy: 55.0,
            znear: 0.1,
            zfar: 100.0,
        }
    }
}

impl fmt::Display for Camera {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Camera [
                eye: {:?},
                target: {:?},
                up: {:?},
                aspect: {},
                fovy: {},
                znear: {},
                zfar: {},
            ]",
            self.eye, self.target, self.up, self.aspect, self.fovy, self.znear, self.zfar
        )
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_position: [f32; 4],
    pub view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_position: [0.0; 4],
            view_proj: Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        // We're using Vector4 because of the uniforms 16 byte spacing requirement
        self.view_position = camera.eye.to_homogeneous().into();
        self.view_proj = (OPENGL_TO_WGPU_MATRIX * camera.build_view_projection_matrix()).into();
    }
}

pub trait CameraController {
    fn input(&mut self, event: GameEvent) -> bool;

    fn update_camera(&self, camera: &mut Camera, screen_size: PhysicalSize<u32>);
}

pub struct FreeCamController {
    speed: f32,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_right_pressed: bool,
    is_left_pressed: bool,
    cursor_delta: Cell<(f64, f64)>,
    look_speed_factor: f64,
    is_up_pressed: bool,
    is_down_pressed: bool,
}

impl Default for FreeCamController {
    fn default() -> Self {
        FreeCamController {
            speed: 0.2,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_right_pressed: false,
            is_left_pressed: false,
            cursor_delta: Cell::new((0.0, 0.0)),
            look_speed_factor: 1.0,
            is_up_pressed: false,
            is_down_pressed: false,
        }
    }
}

impl CameraController for FreeCamController {
    fn input(&mut self, event: GameEvent) -> bool {
        match event {
            GameEvent::CursorMoved { delta, .. } => {
                self.cursor_delta.set(delta);
                false
            }
            GameEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                ..
            } => {
                let is_pressed = state == ElementState::Pressed;
                match keycode {
                    VirtualKeyCode::W | VirtualKeyCode::Up => {
                        self.is_forward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::A | VirtualKeyCode::Left => {
                        self.is_left_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::S | VirtualKeyCode::Down => {
                        self.is_backward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::D | VirtualKeyCode::Right => {
                        self.is_right_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::F => {
                        self.is_up_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::C => {
                        self.is_down_pressed = is_pressed;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn update_camera(&self, camera: &mut Camera, screen_size: PhysicalSize<u32>) {
        let forward = camera.target - camera.eye;
        let forward_norm = forward.normalize();
        let forward_mag = forward.magnitude();

        // Prevents glitching when camera gets too close to the
        // center of the scene.
        if self.is_forward_pressed && forward_mag > self.speed {
            camera.eye += forward_norm * self.speed;
        }
        if self.is_backward_pressed {
            camera.eye -= forward_norm * self.speed;
        }
        // right normal is calculated by doing the cross product between the forward normal and the
        // up normal (check right hand rule)
        let right_vec = forward_norm.cross(camera.up.normalize()) * self.speed;
        if self.is_right_pressed {
            camera.eye += right_vec;
            camera.target += right_vec;
        }
        if self.is_left_pressed {
            camera.eye -= right_vec;
            camera.target -= right_vec;
        }

        // up down movement
        let up_vec = camera.up * self.speed;
        if self.is_up_pressed {
            camera.eye += up_vec;
            camera.target += up_vec;
        }
        if self.is_down_pressed {
            camera.eye -= up_vec;
            camera.target -= up_vec;
        }
        // mouse look:
        let delta = self.cursor_delta.get();
        self.cursor_delta.set((0.0, 0.0));
        let right = forward_norm.cross(camera.up);
        let mut v = (delta.0 as f32 * right) + (delta.1 as f32 * camera.up);
        v *= self.look_speed_factor as f32;
        camera.target += v;

        // todo: camera dampening
        // if (camera.target - camera.eye).y > 1700.0 {
        //     camera.target.y = camera.eye.y + 1600.0;
        // }
    }
}
