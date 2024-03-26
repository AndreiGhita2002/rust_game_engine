use std::collections::HashMap;
use std::mem;
use std::ops::{AddAssign, Deref};

use cgmath::{Matrix2, Matrix4, Quaternion, Vector2, Vector3, Zero};
use wgpu::{BindGroupLayout, Buffer, BufferAddress};
use wgpu::util::DeviceExt;

use crate::{GlobalContext, resources};
use crate::render::model::Model;
use crate::util::{IdManager, QueueBuffer, QueueBufferRef, SharedCell};

pub struct InstanceManager {
    pub models: HashMap<String, Model>,
    pub instances: Vec<Instance>,
    pub instance_3d_buffer: Buffer,
    pub n_3d_buffer: u32,
    pub instance_2d_buffer: Buffer,
    pub n_2d_buffer: u32,
    needs_buffer_remake: bool,
    pub id_manager: IdManager,
}
impl InstanceManager {
    pub fn new(device: &wgpu::Device, id_manager: IdManager) -> Self {
        let instance_3d_data: Vec<Instance3DRaw> = Vec::new();
        let instance_2d_data: Vec<Instance2DRaw> = Vec::new();
        Self {
            // 3D
            models: HashMap::new(),
            instances: Vec::new(),
            instance_3d_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("3D Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_3d_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }),
            instance_2d_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("2D Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_2d_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }),
            n_2d_buffer: 0,
            n_3d_buffer: 0,
            needs_buffer_remake: true,
            id_manager,
        }
    }

    pub fn tick(&mut self, context: &GlobalContext) {
        if self.needs_buffer_remake {
            self.remake_buffer(context);
        } else {
            for instance in self.instances.iter_mut() {
                instance.tick(context, &self.instance_3d_buffer, &self.instance_2d_buffer);
            }
        }
    }

    pub fn register_instance(&mut self, instance_desc: InstanceDesc) -> InstanceRef {
        print!("Registering Instance: {:?}", instance_desc);
        let buf_id;
        match &instance_desc.instance_type {
            &InstanceType::Model => {
                buf_id = self.n_3d_buffer;
                self.n_3d_buffer += 1;
            }
            &InstanceType::Sprite => {
                buf_id = self.n_2d_buffer;
                self.n_2d_buffer += 1;
            }
        }
        println!(" with buffer_id: {buf_id}");
        let instance = Instance {
            instance_type: instance_desc.instance_type,
            change_buffer: QueueBuffer::new(),
            position: instance_desc.position,
            rotation: instance_desc.rotation,
            // todo(feature:Delete) this code makes some assumptions about the id:
            buffer_id: SharedCell::new(buf_id),
        };
        let inst_ref = instance.get_ref();
        self.instances.push(instance);
        self.needs_buffer_remake = true;
        inst_ref
    }

    pub async fn load_model(
        &mut self,
        model_name: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_bind_group_layout: &BindGroupLayout,
    ) -> anyhow::Result<()> {
        let model =
            resources::load_model(model_name, &device, &queue, &texture_bind_group_layout).await?;
        self.models.insert(model_name.to_string(), model);
        anyhow::Ok(())
    }

    pub async fn load_sprite(
        &mut self,
        sprite_name: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_bind_group_layout: &BindGroupLayout,
    ) -> anyhow::Result<()> {
        let sprite = resources::load_sprite(sprite_name, None, &device, &queue, &texture_bind_group_layout).await?;
        self.models.insert(sprite_name.to_string(), sprite);
        anyhow::Ok(())
    }

    pub fn remake_buffer(&mut self, context: &GlobalContext) {
        let mut raw3 = Vec::new();
        let mut raw2 = Vec::new();
        for instance in self.instances.iter() {
            match instance.to_raw() {
                RawInstance::Model(r3) => raw3.push(r3),
                RawInstance::Sprite(r2) => raw2.push(r2),
            }
        }
        self.instance_3d_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("3D Instance Buffer"),
                contents: bytemuck::cast_slice(&raw3),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        self.instance_2d_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("2D Instance Buffer"),
                contents: bytemuck::cast_slice(&raw2),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        self.needs_buffer_remake = false;
    }
}

#[derive(Copy, Clone, Debug)]
pub enum InstanceType {
    Model,
    Sprite,
}

#[derive(Copy, Clone, Debug)]
pub enum InstanceChange {
    PositionSet((f32, f32, f32)),
    PositionAdd((f32, f32, f32)),
    RotationSet((f32, f32, f32, f32)),
    RotationAdd((f32, f32, f32, f32)),
}

pub struct Instance {
    instance_type: InstanceType,
    change_buffer: QueueBuffer<InstanceChange>,
    pub position: Vector3<f32>,
    pub rotation: Quaternion<f32>,
    buffer_id: SharedCell<u32>,
}
impl Instance {
    pub fn tick(&mut self, context: &GlobalContext, instance_buffer_3d: &Buffer, instance_buffer_2d: &Buffer) {
        let changes = self.change_buffer.get_buffer();
        // return if no changes were done to the instance:
        if changes.is_empty() {
            return;
        }

        // changing the position and rotation
        for change in changes {
            match change {
                InstanceChange::PositionSet(pos) => self.position = Vector3::from(pos),
                InstanceChange::PositionAdd(pos) => self.position.add_assign(Vector3::from(pos)),
                InstanceChange::RotationSet(rot) => self.rotation = Quaternion::from(rot),
                InstanceChange::RotationAdd(rot) => self.rotation.add_assign(Quaternion::from(rot)),
            }
        }

        // updating the buffer:
        self.write_to_buffer(context, instance_buffer_3d, instance_buffer_2d);
    }

    fn write_to_buffer(&self, context: &GlobalContext, instance_buffer_3d: &Buffer, instance_buffer_2d: &Buffer) {
        println!("[INST_BUF] writing to buffer for instance {:?} with buffer id: {}",
            self.instance_type, self.buffer_id.borrow()
        );
        let raw = self.to_raw();
        match raw {
            RawInstance::Model(raw_3) => {
                context.queue.write_buffer(
                    instance_buffer_3d,
                    (*self.buffer_id.borrow().deref() * INSTANCE_RAW_3D_SIZE) as BufferAddress,
                    bytemuck::cast_slice(&[raw_3]),
                );
            },
            RawInstance::Sprite(raw_2) => {
                context.queue.write_buffer(
                    instance_buffer_2d,
                    (*self.buffer_id.borrow().deref() * INSTANCE_RAW_2D_SIZE) as BufferAddress,
                    bytemuck::cast_slice(&[raw_2]),
                );
            },
        }
    }

    pub fn get_ref(&self) -> InstanceRef {
        InstanceRef {
            changes_buffer: self.change_buffer.get_ref(),
            gpu_buffer_id: self.buffer_id.clone(),
        }
    }

    pub fn to_raw(&self) -> RawInstance {
        match self.instance_type {
            InstanceType::Model => {
                RawInstance::Model(Instance3DRaw {
                    model: (Matrix4::from_translation(self.position) * Matrix4::from(self.rotation)).into(),
                    normal: cgmath::Matrix3::from(self.rotation).into(),
                })
            },
            InstanceType::Sprite => {
                RawInstance::Sprite(Instance2DRaw {
                    sprite: Matrix2::from_cols(
                        Vector2::new(self.position[0], self.position[1]),
                        Vector2::new(1.0, 1.0),
                    ).into()
                    //  rotation:   * Matrix2::from_angle(self.rotation))
                })
            },
        }

    }
}

#[derive(Clone)]
pub struct InstanceRef {
    pub changes_buffer: QueueBufferRef<InstanceChange>,
    pub gpu_buffer_id: SharedCell<u32>,
}
impl InstanceRef {
    pub fn set_pos(&mut self, pos: (f32, f32, f32)) {
        self.changes_buffer.push(InstanceChange::PositionSet(pos))
    }

    pub fn add_pos(&mut self, pos: (f32, f32, f32)) {
        self.changes_buffer.push(InstanceChange::PositionAdd(pos))
    }

    pub fn set_rot(&mut self, rot: (f32, f32, f32, f32)) {
        self.changes_buffer.push(InstanceChange::RotationSet(rot))
    }

    pub fn add_rot(&mut self, rot: (f32, f32, f32, f32)) {
        self.changes_buffer.push(InstanceChange::RotationAdd(rot))
    }

    pub fn get_instance_id(&self) -> u32 {
        *self.gpu_buffer_id.borrow().deref()
    }
}
#[derive(Copy, Clone, Debug)]
pub struct InstanceDesc {
    pub instance_type: InstanceType,
    pub position: Vector3<f32>,
    pub rotation: Quaternion<f32>,
}

impl Default for InstanceDesc {
    fn default() -> Self {
        InstanceDesc {
            instance_type: InstanceType::Model,
            position: Vector3::zero(),
            rotation: Quaternion::new(1.0, 0.0, 0.0, 0.0),
        }
    }
}

// RAW INSTANCES
pub enum RawInstance {
    Model(Instance3DRaw),
    Sprite(Instance2DRaw),
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance3DRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
}
const INSTANCE_RAW_3D_SIZE: u32 = mem::size_of::<Instance3DRaw>() as u32;

impl Instance3DRaw {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Instance3DRaw>() as BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials we'll
                    // be using 2, 3, and 4, for Vertex. We'll start at slot 5 not conflict with them later
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We'll have to reassemble the mat4 in
                // the shader.
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 19]>() as BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 22]>() as BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance2DRaw {
    sprite: [[f32; 2]; 2],
}
const INSTANCE_RAW_2D_SIZE: u32 = mem::size_of::<Instance2DRaw>() as u32;

impl Instance2DRaw {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Instance2DRaw>() as BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}
