use bevy::{prelude::*, render::mesh};
use byteorder::{LittleEndian, ReadBytesExt};
use rand::{rngs::ThreadRng, Rng};
use std::{
    f32::consts::PI,
    fs::{read_dir, File},
    io::{self, Read},
    path::Path,
};
use warbler_grass::prelude::*;

use crate::{
    assets::{read_faces, read_vertices, read_wedges, ImageAssets, PskxAssets, PSK_FILE_HEADER},
    udp::Ball,
    LoadState,
};

#[derive(Resource)]
pub struct GrassLod(u8);

impl Default for GrassLod {
    fn default() -> Self {
        GrassLod(2)
    }
}

impl GrassLod {
    #[inline]
    pub fn get(&self) -> u8 {
        self.0
    }

    #[inline]
    pub fn set(&mut self, lod: u8) {
        self.0 = lod;
    }
}

#[inline]
fn trim_grass(pos: &Vec3, scale: f32) -> bool {
    // filter out positions inside this triangle
    let p0 = Vec2::new(380. * scale, 385. * scale);
    let p1 = Vec2::new(265. * scale, 500. * scale);
    let p2 = Vec2::new(380. * scale, 500. * scale);

    let p = Vec2::new(pos.x.abs(), pos.z.abs());

    let area = 0.5 * (-p1.y * p2.x + p0.y * (-p1.x + p2.x) + p0.x * (p1.y - p2.y) + p1.x * p2.y);

    let s = 1. / (2. * area) * (p0.y * p2.x - p0.x * p2.y + (p2.y - p0.y) * p.x + (p0.x - p2.x) * p.y);
    let t = 1. / (2. * area) * (p0.x * p1.y - p0.y * p1.x + (p0.y - p1.y) * p.x + (p1.x - p0.x) * p.y);

    !(s > 0. && t > 0. && 1. - s - t > 0.)
}

#[inline]
fn randomize_grass(rand: &mut ThreadRng) -> Vec3 {
    Vec3::new(rand.gen_range(-2.0..2.), 0., rand.gen_range(-2.0..2.))
}

fn generate_grass(scale: i32) -> (Vec<Vec3>, f32, Transform) {
    let mut rand = rand::thread_rng();
    let fscale = scale as f32;

    (
        (-375 * scale..375 * scale)
            .step_by(3)
            .flat_map(|x| (-495 * scale..495 * scale).step_by(3).map(move |z| Vec3::new(x as f32, 1., z as f32)))
            .filter(|pos| trim_grass(pos, fscale))
            .map(|pos| pos + randomize_grass(&mut rand))
            .collect::<Vec<_>>(),
        1.5 * fscale,
        Transform::from_scale(Vec3::splat(10. / fscale)),
    )
}

pub fn get_grass(lod: u8) -> (Vec<Vec3>, f32, Transform) {
    if lod == 0 {
        return (Vec::new(), 1.5, Transform::from_scale(Vec3::splat(10.)));
    }

    generate_grass(lod as i32)
}

pub struct FieldLoaderPlugin;

impl Plugin for FieldLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(WarblersPlugin)
            .insert_resource(GrassLod::default())
            .add_system(load_field.run_if(in_state(LoadState::Field)));
    }
}

fn load_field(
    mut commands: Commands,
    grass_lod: Res<GrassLod>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<NextState<LoadState>>,
    image_assets: Res<ImageAssets>,
    pskx_assets: Res<PskxAssets>,
) {
    // Get all files in ./collision_meshes/soccar/*.cmf
    let raw_mesh = MeshBuilder::combine(
        &read_dir("./collision_meshes/soccar")
            .unwrap()
            .flatten()
            .flat_map(|entry| MeshBuilder::from_file(entry.path()))
            .collect::<Vec<MeshBuilder>>(),
    );

    let inverted_mesh = raw_mesh.clone().invert_indices().build_mesh(50.);
    let mesh = raw_mesh.build_mesh(50.);

    // load the files into the game with their material

    commands.spawn(PbrBundle {
        mesh: meshes.add(mesh),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.2, 0.2, 0.2),
            alpha_mode: AlphaMode::Opaque,
            perceptual_roughness: 0.8,
            reflectance: 0.3,
            ..default()
        }),
        transform: Transform::from_xyz(0., 1., 0.).looking_to(-Vec3::Y, Vec3::Z),
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: meshes.add(inverted_mesh),
        material: materials.add(StandardMaterial {
            base_color: Color::rgba(0.2, 0.2, 0.2, 0.85),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        transform: Transform::from_xyz(0., 1., 0.).looking_to(-Vec3::Y, Vec3::Z),
        ..default()
    });

    // load the side walls

    let mut side_wall_1_transform = Transform::from_xyz(4096., 900., 0.);
    side_wall_1_transform.rotate_local_y(-PI / 2.);
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Quad::new(Vec2::new(7000., 1200.)))),
        material: materials.add(StandardMaterial {
            base_color: Color::rgba(0.2, 0.2, 0.2, 0.9),
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            perceptual_roughness: 0.3,
            reflectance: 0.7,
            ..default()
        }),
        transform: side_wall_1_transform,
        ..default()
    });

    let mut side_wall_2_transform = Transform::from_xyz(-4096., 900., 0.);
    side_wall_2_transform.rotate_local_y(PI / 2.);
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Quad::new(Vec2::new(7000., 1200.)))),
        material: materials.add(StandardMaterial {
            base_color: Color::rgba(0.2, 0.2, 0.2, 0.9),
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            perceptual_roughness: 0.3,
            reflectance: 0.7,
            ..default()
        }),
        transform: side_wall_2_transform,
        ..default()
    });

    // load floor

    let mut floor_transform = Transform::default();
    floor_transform.rotate_local_x(-PI / 2.);
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Quad::new(Vec2::new(7500., 10800.)))),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.7, 1., 0.7),
            perceptual_roughness: 0.9,
            reflectance: 0.05,
            ..default()
        }),
        transform: floor_transform,
        ..default()
    });

    // load ceiling

    let ceiling_material = StandardMaterial {
        base_color: Color::rgba(0.2, 0.2, 0.2, 0.85),
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        perceptual_roughness: 0.3,
        reflectance: 0.7,
        ..default()
    };

    let mut ceiling_transform = Transform::from_xyz(0., 2060., 0.);
    ceiling_transform.rotate_local_x(PI / 2.);
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Quad::new(Vec2::new(6950., 8725.)))),
        material: materials.add(ceiling_material),
        transform: ceiling_transform,
        ..default()
    });

    // load grass

    let (positions, height, transform) = get_grass(grass_lod.get());

    commands.spawn(WarblersExplicitBundle {
        grass: Grass::new(positions, height),
        spatial: SpatialBundle { transform, ..default() },
        ..default()
    });

    // load a glowing ball

    let initial_ball_color = Color::rgb(0.3, 0.3, 0.3);

    let ball_material = StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(image_assets.ball.clone()),
        // normal_map_texture: Some(image_assets.ball_normal.clone()),
        emissive: Color::rgb(0.02, 0.02, 0.02),
        emissive_texture: Some(image_assets.ball_emissive.clone()),
        perceptual_roughness: 0.4,
        metallic: 0.,
        ..default()
    };

    commands
        .spawn((
            Ball,
            PbrBundle {
                mesh: pskx_assets.ball.clone(),
                material: materials.add(ball_material),
                transform: Transform::from_xyz(0., 92., 0.),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn(PointLightBundle {
                point_light: PointLight {
                    color: initial_ball_color,
                    radius: 98.,
                    shadows_enabled: true,
                    intensity: 2_000_000.,
                    range: 1000.,
                    ..default()
                },
                ..default()
            });
        });

    state.set(LoadState::Connect);
}

/// A collection of inter-connected triangles.
#[derive(Clone, Debug, Default)]
pub struct MeshBuilder {
    ids: Vec<u32>,
    verts: Vec<f32>,
    uvs: Vec<Vec2>,
}

impl MeshBuilder {
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = File::open(path)?;

        let ids_len = file.read_i32::<LittleEndian>()? * 3;
        let verts_len = file.read_i32::<LittleEndian>()? * 3;

        let ids = (0..ids_len).map(|_| file.read_i32::<LittleEndian>().map(|x| x as u32)).collect::<io::Result<Vec<_>>>()?;
        let verts = (0..verts_len - verts_len % 3).map(|_| file.read_f32::<LittleEndian>()).collect::<io::Result<Vec<_>>>()?;

        // Verify that the triangle data is correct
        let max_vert = verts.len() as u32 / 3;
        for &id in &ids {
            if id >= max_vert {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid triangle data"));
            }
        }

        Ok(Self { ids, verts, uvs: Vec::new() })
    }

    #[must_use]
    /// Combine different meshes all into one
    pub fn combine(other_meshes: &[Self]) -> Self {
        let n_ids = other_meshes.iter().map(|mesh| mesh.ids.len()).sum();
        let mut ids: Vec<u32> = Vec::with_capacity(n_ids);
        let mut id_offset = 0;

        for m in other_meshes {
            ids.extend(m.ids.iter().map(|id| id + id_offset));
            id_offset += m.verts.len() as u32 / 3;
        }

        let verts: Vec<f32> = other_meshes.iter().flat_map(|m| m.verts.clone()).collect();

        let uvs = Vec::new();

        Self { ids, verts, uvs }
    }

    #[must_use]
    pub fn invert_indices(mut self) -> Self {
        self.ids.chunks_exact_mut(3).for_each(|chunk| chunk.swap(1, 2));
        self
    }

    #[must_use]
    // Build the Bevy Mesh
    pub fn build_mesh(self, scale: f32) -> Mesh {
        let mut mesh = Mesh::new(mesh::PrimitiveTopology::TriangleList);

        let verts = self
            .verts
            .chunks_exact(3)
            .map(|chunk| [chunk[0] * scale, chunk[1] * scale, chunk[2] * scale])
            .collect::<Vec<_>>();

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.ids.iter().map(|&id| verts[id as usize]).collect::<Vec<_>>());

        mesh.compute_flat_normals();

        if !self.uvs.is_empty() {
            // duplicate uvs like verts & ids
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);

            // compute tangents
            mesh.set_indices(Some(mesh::Indices::U32(
                (0..mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len() as u32).collect::<Vec<_>>(),
            )));
            mesh.generate_tangents().unwrap();
        }

        mesh
    }

    /// Create a mesh from a Rocket League .pskx file
    pub fn from_pskx(bytes: &[u8]) -> Result<Self, bevy::asset::Error> {
        let mut cursor = io::Cursor::new(bytes);

        // ensure file header matches PSK_FILE_HEADER
        let mut file_header = [0; 32];
        cursor.read_exact(&mut file_header)?;
        assert_eq!(&file_header[..PSK_FILE_HEADER.len()], PSK_FILE_HEADER);

        let mut ids = Vec::new();
        let mut verts = Vec::new();
        let mut uvs = Vec::new();

        let mut wedges = Vec::new();

        // read chunks
        loop {
            let mut chunk_header = [0; 32];
            if cursor.read_exact(&mut chunk_header).is_err() {
                break;
            }

            let chunk_id = std::str::from_utf8(&chunk_header[0..8])?;
            // let chunk_type = i32::from_le_bytes([chunk_header[20], chunk_header[21], chunk_header[22], chunk_header[23]]);
            let chunk_data_size = i32::from_le_bytes([chunk_header[24], chunk_header[25], chunk_header[26], chunk_header[27]]) as usize;
            let chunk_data_count = i32::from_le_bytes([chunk_header[28], chunk_header[29], chunk_header[30], chunk_header[31]]) as usize;

            if chunk_data_count == 0 {
                continue;
            }

            let mut chunk_data = vec![0; chunk_data_size * chunk_data_count];
            cursor.read_exact(&mut chunk_data)?;

            // use plenty of debug asserts to ensure valid data processing
            // with no performance impact in release mode
            match chunk_id {
                "PNTS0000" => {
                    verts = read_vertices(&chunk_data, chunk_data_count);
                    debug_assert_eq!(verts.len() / 3, chunk_data_count);
                    debug_assert_eq!(verts.len() % 3, 0);

                    verts.truncate(verts.len() - verts.len() / 3 % 3 * 3);
                    debug_assert_eq!(verts.len(), chunk_data_count * 3 - chunk_data_count % 3 * 3);
                    debug_assert_eq!(verts.len() / 3 % 3, 0);
                }
                "VTXW0000" => {
                    wedges = read_wedges(&chunk_data, chunk_data_count);
                    debug_assert_eq!(wedges.len(), chunk_data_count);
                }
                "FACE0000" => {
                    let mut faces = read_faces(&chunk_data, chunk_data_count, &wedges);
                    debug_assert_eq!(faces.len(), chunk_data_count);

                    // remove faces that reference invalid verts in chunks of 3 faces
                    let max_vert = verts.len() as u32 / 3;
                    faces.retain(|face| face.iter().all(|(id, _)| *id < max_vert));

                    (ids, uvs) = faces.into_iter().flatten().unzip();
                    debug_assert_eq!(ids.len() / 3 % 3, 0);
                    debug_assert_eq!(ids.len(), uvs.len());
                    debug_assert_eq!(ids.iter().max().unwrap(), &(verts.len() as u32 / 3 - 1));
                }
                "MATT0000" => assert_eq!(chunk_data_count, 1),
                _ => {
                    println!("Unknown chunk: {}", chunk_id);
                }
            }
        }

        Ok(Self { ids, verts, uvs })
    }
}
