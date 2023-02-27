use std::f32::consts::PI;
use bevy_readback::{
    ComputeError, ComputeRequest, ComputeRequestToken, ReadbackComponent, ReadbackComponentPlugin,
    ReadbackPlugin,
};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_resource::{BufferSize, ShaderType, TextureDimension};
use bevy::render::RenderApp;
use bevy::utils::HashMap;
use bevy::{
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    prelude::*,
    render::{
        render_resource::{
            BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry,
            Buffer, BufferDescriptor, BufferInitDescriptor, BufferUsages, ShaderStages,
        },
        renderer::RenderDevice,
    },
};
use serde::Serialize;
use wgpu::{Extent3d, TextureFormat, TextureUsages};

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugin(ReadbackPlugin::whenever());
    // add plugin per required request type
    app.add_plugin(ReadbackComponentPlugin::<GpuRequest>::default());

    app.add_plugin(ExtractResourcePlugin::<ImageResource>::default());

    app.insert_resource(ImageResource {
        image: Handle::default(),
    });
    app.add_system(update_image);
    app.add_system(run_compute_requests);
    app.add_startup_system(setup);
    app.run()
}

pub const CHUNK_SIZE: usize = 128;
pub const BUFFER_SIZE: usize = CHUNK_SIZE * CHUNK_SIZE;

#[derive(Resource, ExtractResource, Clone)]
struct ImageResource {
    image: Handle<Image>,
}

#[derive(Component)]
struct Marker;

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut image = Image::new_fill(
        Extent3d {
            width: 512,
            height: 512,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255, 0, 0, 255],
        TextureFormat::Rgba8Unorm,
    );
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
    let image = images.add(image);

    let material_handle = materials.add(StandardMaterial {
        base_color_texture: Some(image),
        ..default()
    });

    let cube_handle = meshes.add(Mesh::from(shape::Quad {
        size: Vec2::splat(8.0),
        flip: false,
    }));

    commands.spawn((
        PbrBundle {
            mesh: cube_handle,
            material: material_handle,
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        Marker,
    ));

    // camera
    commands
        .spawn(Camera3dBundle {
            transform: Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        });

    // light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform {
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
        ..default()
    });

}

fn update_image(
    mut commands: Commands,
    mut image_res: ResMut<ImageResource>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<Entity, With<Marker>>,
) {
    if image_res.is_changed() {
        for entity in query.iter() {
            let mut a = commands.entity(entity);

            let material_handle = materials.add(StandardMaterial {
                base_color_texture: Some(image_res.image.clone()),
                ..default()
            });
            a.insert(material_handle);
        }
    }
}

#[derive(ShaderType, Clone)]
pub struct MyArray {
    data: [Vec4; BUFFER_SIZE],
}

#[derive(Clone, Copy, ShaderType, Serialize)]
#[repr(C)]
pub struct ShaderInput {
    coord: Vec2,
}

impl Default for MyArray {
    fn default() -> Self {
        MyArray {
            data: [Vec4::splat(0.0); BUFFER_SIZE],
        }
    }
}

fn run_compute_requests(
    mut req: ComputeRequest<GpuRequest>,
    mut token: Local<HashMap<usize, ComputeRequestToken<GpuRequest>>>,
    mut data: Local<HashMap<usize, MyArray>>,
    mut image_res: ResMut<ImageResource>,
    mut images: ResMut<Assets<Image>>,
) {
    for x in [1] {
        if let Some(t) = token.get(&x) {
            match req.try_get(*t) {
                Ok(res) => {
                    token.remove(&x);
                    let mut image = Image::new_fill(
                        Extent3d {
                            width: CHUNK_SIZE as u32,
                            height: CHUNK_SIZE as u32,
                            depth_or_array_layers: 1,
                        },
                        TextureDimension::D2,
                        &[
                            0.0_f32.to_le_bytes(),
                            0.0_f32.to_le_bytes(),
                            0.0_f32.to_le_bytes(),
                            1.0_f32.to_le_bytes(),
                        ]
                            .concat(),
                        TextureFormat::Rgba32Float,
                    );
                    dbg!(&res.data.iter().take(32).collect::<Vec<&Vec4>>());
                    // Why is an image with rgba32Float maxed at 0.255 instead of 1.0?
                    for (index, vec) in res.data.iter().enumerate() {
                        let i = index * 4 * 4;
                        image.data[i..i + 4].copy_from_slice(&vec.x.to_le_bytes());
                        image.data[i + 4..i + 8].copy_from_slice(&vec.y.to_le_bytes());
                        image.data[i + 8..i + 12].copy_from_slice(&vec.z.to_le_bytes());
                    }
                    let image = images.add(image);
                    image_res.image = image.clone();
                }
                Err(ComputeError::NotReady) => {
                    println!("not ready");
                }
                Err(ComputeError::Failed) => panic!(),
            }
        }
        if data.get(&x).is_some() {
            continue;
        }

        if token.get(&x).is_none() {
            data.insert(x, MyArray::default());
            token.insert(x, req.request(ShaderInput { coord: Vec2::splat(0.10) }));
            info!(
                "[{}] making request for {:?}",
                x,
                data.get(&x).unwrap().data.len()
            );
        }
    }
}

#[derive(Component)]
pub struct GpuRequest {
    #[allow(dead_code)]
    input_buffer: Buffer,
    output_buffer: Buffer,
    bindgroup: BindGroup,
}

// implement readback trait
impl ReadbackComponent for GpuRequest {
    // input data
    type SourceData = ShaderInput;
    // data required in render world
    type RenderData = ShaderInput;
    // return type (must implement ShaderType, no runtime size elements)
    type Result = MyArray;

    // system param for prepare function
    type PrepareParam = SRes<RenderDevice>;

    // cheap extract from main world to render world
    fn extract(data: &Self::SourceData) -> Self::RenderData {
        data.clone()
    }

    // build buffers etc; can use resources for persistent buffers
    fn prepare(
        render_data: Self::RenderData,
        layout: &BindGroupLayout,
        device: &SystemParamItem<Self::PrepareParam>,
    ) -> Self {
        let input_buffer = device.create_buffer_with_data(&BufferInitDescriptor {
            label: None,
            usage: BufferUsages::UNIFORM,
            contents: &bincode::serialize(&render_data.coord).unwrap(),
        });

        let output_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: (BUFFER_SIZE * 16) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, // note: COPY_SRC is required for the output buffer
            mapped_at_creation: false,
        });

        let bindgroup = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            input_buffer,
            output_buffer,
            bindgroup,
        }
    }

    // return the bind group created in prepare
    fn bind_group(&self) -> BindGroup {
        self.bindgroup.clone()
    }

    // return a reference to the output buffer to be read back
    fn readback_source(&self) -> Buffer {
        self.output_buffer.clone()
    }

    // compute shader ref
    fn shader() -> bevy::render::render_resource::ShaderRef {
        "double.wgsl".into()
    }

    // entry point
    fn entry_point() -> std::borrow::Cow<'static, str> {
        "double".into()
    }

    // vec of layout entries for the shader, used in pipeline FromWorld at app startup
    fn bind_group_layout_entries() -> Vec<bevy::render::render_resource::BindGroupLayoutEntry> {
        vec![
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: bevy::render::render_resource::BindingType::Buffer {
                    ty: bevy::render::render_resource::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(ShaderInput::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: bevy::render::render_resource::BindingType::Buffer {
                    ty: bevy::render::render_resource::BufferBindingType::Storage {
                        read_only: false,
                    },
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(0),
                },
                count: None,
            },
        ]
    }
}
