use std::collections::HashMap;
use std::mem;
use std::ops::{AddAssign, Deref};

use cgmath::{Matrix4, Quaternion, Vector3, Zero};
use wgpu::{BindGroupLayout, Buffer, BufferAddress};
use wgpu::util::DeviceExt;

use crate::{GlobalContext, resources};
use crate::render::model::Model;
use crate::util::{IdManager, QueueBuffer, QueueBufferRef, SharedCell};

pub struct InstanceManager {
    // 3D
    pub models: HashMap<String, Model>,
    pub instances: Vec<Instance>,
    pub instance_3d_buffer: Buffer,
    needs_buffer_remake: bool,
    // 2D
    //pub sprites: HashMap<String, Model>,
    //instances_2d: Vec<Instance2D>,
    //pub instance_2d_buffer: Buffer,
    // id manager:
    pub id_manager: IdManager,
}
impl InstanceManager {
    pub fn new(device: &wgpu::Device, id_manager: IdManager) -> Self {
        let instance_3d_data: Vec<Instance3DRaw> = Vec::new();
        Self {
            // 3D
            models: HashMap::new(),
            instances: Vec::new(),
            instance_3d_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_3d_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }),
            needs_buffer_remake: false,
            id_manager,
        }
    }

    pub fn tick(&mut self, context: &GlobalContext) {
        if self.needs_buffer_remake {
            self.remake_buffer(context);
        } else {
            for instance in self.instances.iter_mut() {
                instance.tick(context, &self.instance_3d_buffer);
            }
        }
    }

    pub fn register_instance(&mut self, instance_desc: InstanceDesc) -> InstanceRef {
        println!("Registering Instance: {:?}", instance_desc);
        let instance = Instance {
            instance_type: instance_desc.instance_type,
            change_buffer: QueueBuffer::new(),
            position: instance_desc.position,
            rotation: instance_desc.rotation,
            // todo(feature:Delete) this code makes some assumptions about the id:
            buffer_id: SharedCell::new(self.instances.len() as u32),
        };
        let inst_ref = instance.get_ref();
        self.instances.push(instance);
        self.needs_buffer_remake = true;
        inst_ref
    }

    pub async fn load_model(
        &mut self,
        file_name: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_bind_group_layout: &BindGroupLayout,
    ) -> anyhow::Result<()> {
        let model =
            resources::load_model(file_name, &device, &queue, &texture_bind_group_layout).await?;
        self.models.insert(file_name.to_string(), model);
        anyhow::Ok(())
    }

    pub fn remake_buffer(&mut self, context: &GlobalContext) {
        let instance_data = self
            .instances
            .iter()
            .map(|inst| inst.to_raw())
            .collect::<Vec<_>>();
        let instance_buffer =
            context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&instance_data),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });
        self.instance_3d_buffer = instance_buffer;
        self.needs_buffer_remake = false;
    }
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
pub enum InstanceType {
    Model,
    Sprite,
    ScreenSpace,
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
    pub fn tick(&mut self, context: &GlobalContext, instance_buffer: &Buffer) {
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
        context.queue.write_buffer(
            instance_buffer,
            (*self.buffer_id.borrow().deref() * INSTANCE_RAW_SIZE) as BufferAddress,
            bytemuck::cast_slice(&[self.to_raw()]),
        );
    }

    pub fn get_ref(&self) -> InstanceRef {
        InstanceRef {
            changes_buffer: self.change_buffer.get_ref(),
            gpu_buffer_id: self.buffer_id.clone(),
        }
    }

    pub fn to_raw(&self) -> Instance3DRaw {
        Instance3DRaw {
            model: (Matrix4::from_translation(self.position) * Matrix4::from(self.rotation)).into(),
            normal: cgmath::Matrix3::from(self.rotation).into(),
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

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance3DRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
}
const INSTANCE_RAW_SIZE: u32 = mem::size_of::<Instance3DRaw>() as u32;

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
