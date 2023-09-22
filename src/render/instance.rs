use std::collections::HashMap;

use cgmath::{Matrix4, Quaternion, Vector3};
use wgpu::{BindGroupLayout, Buffer};
use wgpu::util::DeviceExt;

use crate::{GlobalContext, resources};
use crate::render::model::Model;
use crate::util::{IdManager, SharedCell};

pub struct InstanceManager {
    // 3D
    pub models: HashMap<String, Model>,
    pub instances_3d: Vec<SharedCell<Instance3D>>,
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
            instances_3d: Vec::new(),
            instance_3d_buffer: device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("3D Instance Buffer"),
                    contents: bytemuck::cast_slice(&instance_3d_data),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                }
            ),
            needs_buffer_remake: false,
            id_manager,
        }
    }

    pub fn tick(&mut self, context: &GlobalContext) {
        //todo use queue.write_to_buffer() instead of remaking the buffer every frame
        // example:
        //    context.queue.write_buffer(&self.instance_3d_buffer, 10, &data);

        self.needs_buffer_remake = true;
        if self.needs_buffer_remake {
            self.remake_buffer(context);
        }
    }

    pub fn register_instance(&mut self, instance: SharedCell<Instance3D>) -> usize {
        self.instances_3d.push(instance);
        self.needs_buffer_remake = true;
        self.instances_3d.len() - 1
    }

    pub async fn load_model(&mut self, file_name: &str, device: &wgpu::Device, queue: &wgpu::Queue,
                            texture_bind_group_layout: &BindGroupLayout) -> anyhow::Result<()> {
        let model = resources::load_model(file_name, &device, &queue, &texture_bind_group_layout)
            .await?;
        self.models.insert(file_name.to_string(), model);
        anyhow::Ok(())
    }

    pub fn remake_buffer(&mut self, context: &GlobalContext) {
        let instance_data = self.instances_3d
            .iter()
            .map(|inst| inst.borrow().to_raw())
            .collect::<Vec<_>>();
        let instance_buffer = context.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }
        );
        self.instance_3d_buffer = instance_buffer;
        self.needs_buffer_remake = false;
    }
}


// 3D Instance
#[derive(Clone, Debug)]
pub struct Instance3D {
    pub position: Vector3<f32>,
    pub rotation: Quaternion<f32>,
    pub model_name: String,
}
impl Instance3D {
    pub fn to_raw(&self) -> Instance3DRaw {
        Instance3DRaw {
            model: (Matrix4::from_translation(self.position) * Matrix4::from(self.rotation))
                .into(),
            normal: cgmath::Matrix3::from(self.rotation).into(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance3DRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
}
impl Instance3DRaw {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Instance3DRaw>() as wgpu::BufferAddress,
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
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
                },

            ],
        }
    }
}
