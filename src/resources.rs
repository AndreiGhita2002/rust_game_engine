use std::io::{BufReader, Cursor};

use cfg_if::cfg_if;
use wgpu::{BindGroupLayout, Device, Queue};
use wgpu::util::DeviceExt;

use crate::render::{model, texture};
use crate::render::model::{Material, Mesh, ModelVertex, SpriteVertex};

#[cfg(target_arch = "wasm32")]
fn format_url(file_name: &str) -> reqwest::Url {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let mut origin = location.origin().unwrap();
    if !origin.ends_with("game_attempt_3/res") {
        origin = format!("{}/game_attempt_3/res", origin);
    }
    let base = reqwest::Url::parse(&format!("{}/", origin,)).unwrap();
    let out = base.join(file_name).unwrap();
    println!("format_url({}) -> {}", file_name, out);
    out
}

const MODEL_DIR: &'static str = "models/";

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let txt = reqwest::get(url)
                .await?
                .text()
                .await?;
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("res")
                .join(file_name);
            let txt = std::fs::read_to_string(path)?;
        }
    }
    Ok(txt)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let data = reqwest::get(url)
                .await?
                .bytes()
                .await?
                .to_vec();
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("res")
                .join(file_name);
            let data = std::fs::read(path)?;
        }
    }
    Ok(data)
}

pub async fn load_texture(
    file_name: &str,
    device: &Device,
    queue: &Queue,
) -> anyhow::Result<texture::Texture> {
    let data = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name)
}

pub async fn load_model(
    model_name: &str,
    device: &Device,
    queue: &Queue,
    layout: &BindGroupLayout,
) -> anyhow::Result<model::Model> {
    let obj_url = format!("{MODEL_DIR}{model_name}.obj");
    let obj_text = load_string(&obj_url).await?;
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            let material_url = format!("{MODEL_DIR}{p}");
            let mat_text = load_string(&material_url).await.unwrap();
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )
    .await?;

    let mut materials = Vec::new();
    for m in obj_materials? {
        let texture_url = format!("{MODEL_DIR}{}", m.diffuse_texture.unwrap());
        let diffuse_texture = load_texture(&texture_url, device, queue).await?;
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: None,
        });

        materials.push(Material {
            name: m.name,
            diffuse_texture,
            bind_group,
        })
    }

    let meshes = models
        .into_iter()
        .map(|m| {
            let vertices = (0..m.mesh.positions.len() / 3)
                .map(|i| ModelVertex {
                    position: [
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ],
                    tex_coords: [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]],
                    normal: [
                        m.mesh.normals[i * 3],
                        m.mesh.normals[i * 3 + 1],
                        m.mesh.normals[i * 3 + 2],
                    ],
                })
                .collect::<Vec<_>>();

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Vertex Buffer", model_name)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Index Buffer", model_name)),
                contents: bytemuck::cast_slice(&m.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            Mesh {
                name: model_name.to_string(),
                vertex_buffer,
                index_buffer,
                num_elements: m.mesh.indices.len() as u32,
                material: m.mesh.material_id.unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

    Ok(model::Model { meshes, materials })
}

pub async fn load_sprite(
    sprite_name: &str,
    vertices: Option<Vec<SpriteVertex>>,
    device: &Device,
    queue: &Queue,
    layout: &BindGroupLayout,
) -> anyhow::Result<model::Model> {
    let file_url = format!("{MODEL_DIR}{sprite_name}.jpg");  //todo sprites can only be jpg rn
    let indices: Vec<u32> = vec![0, 1, 1, 2, 2, 3, 3, 0];
    let vert = vertices.unwrap_or(vec![
        SpriteVertex { position: [1.0, 1.0], tex_coords: [1.0, 1.0] },
        SpriteVertex { position: [1.0, 0.0], tex_coords: [1.0, 0.0] },
        SpriteVertex { position: [0.0, 0.0], tex_coords: [0.0, 0.0] },
        SpriteVertex { position: [0.0, 1.0], tex_coords: [0.0, 1.0] },
    ]);
    let diffuse_texture = load_texture(&file_url, device, queue).await?;
    // todo: use the size of the texture:
    // let ratio = diffuse_texture.texture.height() as f32 / diffuse_texture.texture.width() as f32;
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
            },
        ],
        label: None,
    });
    Ok(model::Model {
        meshes: vec![Mesh::from_vertices(
            vert, indices, sprite_name, None, device,
        )],
        materials: vec![Material {
            name: sprite_name.to_string(),
            diffuse_texture,
            bind_group,
        }],
    })
}
