use bevy::{prelude::*, render::mesh};
use rand::{rngs::ThreadRng, Rng};
use std::{
    f32::consts::PI,
    io::{self, Read},
};
use warbler_grass::prelude::*;

use crate::{assets::*, udp::Ball, LoadState};

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
    let p0 = Vec2::new(385. * scale, 380. * scale);
    let p1 = Vec2::new(265. * scale, 495. * scale);
    let p2 = Vec2::new(385. * scale, 495. * scale);

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
        (-380 * scale..380 * scale)
            .step_by(3)
            .flat_map(|x| (-483 * scale..483 * scale).step_by(3).map(move |z| Vec3::new(x as f32, 1., z as f32)))
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
    core_assets: Res<CoreAssets>,
    dfh_stadium: Res<DfhStadium>,
) {
    commands.spawn(PbrBundle {
        mesh: dfh_stadium.oob_floor.clone(),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.3, 0.3, 0.3),
            metallic: 0.1,
            ..default()
        }),
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: dfh_stadium.oob_floor_trim.clone(),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.3, 0.3, 0.3),
            normal_map_texture: Some(dfh_stadium.brushed_metal_normal.clone()),
            emissive: Color::rgb(0.1, 0.1, 0.1),
            metallic: 0.8,
            ..default()
        }),
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: dfh_stadium.field_center.clone(),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.9, 0.3, 0.3),
            metallic: 0.1,
            ..default()
        }),
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: dfh_stadium.field_center_lines.clone(),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.9, 0.3, 0.3),
            metallic: 0.1,
            ..default()
        }),
        ..default()
    });

    // commands.spawn(PbrBundle {
    //     mesh: dfh_stadium.field_center_trim.clone(),
    //     material: materials.add(StandardMaterial {
    //         base_color: Color::rgb(0.9, 0.3, 0.3),
    //         metallic: 0.1,
    //         cull_mode: None,
    //         double_sided: true,
    //         ..default()
    //     }),
    //     ..default()
    // });

    commands.spawn(PbrBundle {
        mesh: dfh_stadium.field_center_field_team1.clone(),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.9, 0.3, 0.3),
            metallic: 0.1,
            ..default()
        }),
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: dfh_stadium.field_center_field_team2.clone(),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.9, 0.3, 0.3),
            metallic: 0.1,
            ..default()
        }),
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: dfh_stadium.field_center_vent.clone(),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.9, 0.3, 0.3),
            metallic: 0.1,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        ..default()
    });

    let ff_cage_full = meshes.get(&dfh_stadium.ff_cage_full.clone()).unwrap().flip();
    let inv_ff_cage_full = meshes.add(ff_cage_full);
    let ff_cage_full_mat = materials.add(StandardMaterial {
        base_color: Color::rgb(0.3, 0.3, 0.3),
        metallic: 0.1,
        cull_mode: None,
        double_sided: true,
        ..default()
    });

    let ff_goal = meshes.get(&dfh_stadium.ff_goal.clone()).unwrap().flip();
    let inv_ff_goal = meshes.add(ff_goal);

    let ff_roof = meshes.get(&dfh_stadium.ff_roof.clone()).unwrap().flip();
    let inv_ff_roof = meshes.add(ff_roof);

    let ff_side = meshes.get(&dfh_stadium.ff_side.clone()).unwrap().flip();
    let inv_ff_side = meshes.add(ff_side);

    let glass_mat = materials.add(StandardMaterial {
        base_color: Color::rgb(0.3, 0.3, 0.3),
        base_color_texture: Some(core_assets.hexagons_pack.clone()),
        metallic: 0.1,
        cull_mode: None,
        double_sided: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let goal_lines = meshes.get(&dfh_stadium.goal_lines.clone()).unwrap().flip();
    let inv_goal_lines = meshes.add(goal_lines);
    let goal_lines_mat = materials.add(StandardMaterial {
        base_color: Color::rgb(0.3, 0.3, 0.9),
        metallic: 0.1,
        ..default()
    });

    let goal_glass_mat = materials.add(StandardMaterial {
        base_color: Color::rgba(0.3, 0.3, 0.3, 0.5),
        metallic: 0.1,
        cull_mode: None,
        double_sided: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let field_frame_outer = meshes.get(&dfh_stadium.field_frame_outer.clone()).unwrap().flip();
    let inv_field_frame_outer = meshes.add(field_frame_outer);
    let field_frame_outer_mat = materials.add(StandardMaterial {
        base_color: Color::rgb(0.3, 0.3, 0.3),
        metallic: 0.1,
        cull_mode: None,
        double_sided: true,
        ..default()
    });

    let goal_std_trim_mat = materials.add(StandardMaterial {
        base_color: Color::rgb(0.3, 0.3, 0.3),
        metallic: 0.1,
        ..default()
    });

    let field_std_frame_mat = materials.add(StandardMaterial {
        base_color: Color::rgb(0.3, 0.3, 0.3),
        metallic: 0.1,
        ..default()
    });

    let goal_std_floor_mat = materials.add(StandardMaterial {
        base_color: Color::rgb(0.9, 0.3, 0.3),
        metallic: 0.1,
        ..default()
    });

    let field_std_trim_mat = materials.add(StandardMaterial {
        base_color: Color::rgb(0.9, 0.3, 0.3),
        metallic: 0.1,
        ..default()
    });

    let field_std_trim_b_mat = materials.add(StandardMaterial {
        base_color: Color::rgb(0.3, 0.3, 0.3),
        normal_map_texture: Some(dfh_stadium.brushed_metal_normal.clone()),
        emissive: Color::rgb(0.1, 0.1, 0.1),
        metallic: 0.8,
        ..default()
    });

    let side_trim = meshes.get(&dfh_stadium.side_trim.clone()).unwrap().flip();
    let inv_side_trim = meshes.add(side_trim);
    let side_trim_mat = materials.add(StandardMaterial {
        base_color: Color::rgb(0.3, 0.9, 0.3),
        metallic: 0.1,
        cull_mode: None,
        double_sided: true,
        ..default()
    });

    let mut stadium_transform = Transform::default();
    for i in (-1..=1).step_by(2) {
        commands.spawn(PbrBundle {
            mesh: dfh_stadium.side_trim.clone(),
            material: side_trim_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: inv_side_trim.clone(),
            material: side_trim_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.field_frame_outer.clone(),
            material: field_frame_outer_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: inv_field_frame_outer.clone(),
            material: field_frame_outer_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.goal_lines.clone(),
            material: goal_lines_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: inv_goal_lines.clone(),
            material: goal_lines_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.field_std_frame.clone(),
            material: field_std_frame_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.ff_cage_full.clone(),
            material: ff_cage_full_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: inv_ff_cage_full.clone(),
            material: ff_cage_full_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.ff_goal.clone(),
            material: glass_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: inv_ff_goal.clone(),
            material: glass_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.ff_roof.clone(),
            material: glass_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: inv_ff_roof.clone(),
            material: glass_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.ff_side.clone(),
            material: glass_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: inv_ff_side.clone(),
            material: glass_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        let goal_transform = Transform::from_xyz(0., 0., -5120. * i as f32).with_rotation(stadium_transform.rotation);

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.goal_std_trim.clone(),
            material: goal_std_trim_mat.clone(),
            transform: goal_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.goal_std_frame.clone(),
            material: goal_glass_mat.clone(),
            transform: goal_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.goal_std_quarterpipe.clone(),
            material: goal_std_trim_mat.clone(),
            transform: goal_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.goal_std_glass.clone(),
            material: goal_glass_mat.clone(),
            transform: goal_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.goal_std_floor.clone(),
            material: goal_std_floor_mat.clone(),
            transform: goal_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.field_std_trim.clone(),
            material: field_std_trim_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        commands.spawn(PbrBundle {
            mesh: dfh_stadium.field_std_trim_b.clone(),
            material: field_std_trim_b_mat.clone(),
            transform: stadium_transform,
            ..default()
        });

        stadium_transform.rotate_local_y(PI);
    }

    commands.spawn(PbrBundle {
        mesh: dfh_stadium.field_std_floor_hex.clone(),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0., 0.3, 0.9),
            metallic: 0.1,
            ..default()
        }),
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: dfh_stadium.boost_pads_01_combined.clone(),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.3, 0.9, 0.3),
            metallic: 0.1,
            ..default()
        }),
        transform: Transform::from_xyz(0., 0., -3250.),
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: dfh_stadium.boost_pads_02_combined.clone(),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.3, 0.9, 0.3),
            metallic: 0.1,
            ..default()
        }),
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: dfh_stadium.boost_pads_03_combined.clone(),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.3, 0.9, 0.3),
            metallic: 0.1,
            ..default()
        }),
        transform: Transform::from_xyz(0., 0., 3250.),
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
        base_color_texture: Some(core_assets.ball_diffuse.clone()),
        normal_map_texture: Some(core_assets.ball_normal.clone()),
        emissive: Color::rgb(0.02, 0.02, 0.02),
        emissive_texture: Some(core_assets.ball_emissive.clone()),
        perceptual_roughness: 0.4,
        metallic: 0.,
        unlit: true,
        ..default()
    };

    commands
        .spawn((
            Ball,
            PbrBundle {
                mesh: core_assets.ball.clone(),
                material: materials.add(ball_material),
                transform: Transform::from_xyz(0., 92., 0.),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn(PointLightBundle {
                point_light: PointLight {
                    color: initial_ball_color,
                    radius: 110.,
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

trait Flip {
    fn flip(&self) -> Self;
}

impl Flip for Mesh {
    fn flip(&self) -> Self {
        let mut ids = self.indices().unwrap().iter().map(|i| i as u32).collect::<Vec<_>>();
        ids.chunks_exact_mut(3).for_each(|c| c.swap(1, 2));

        let verts = self
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap()
            .iter()
            .flat_map(|v| [v[0], v[1], -v[2]])
            .collect::<Vec<_>>();

        let uvs = if let Some(mesh::VertexAttributeValues::Float32x2(values)) = self.attribute(Mesh::ATTRIBUTE_UV_0) {
            values.clone()
        } else {
            Vec::new()
        };

        let colors = if let Some(mesh::VertexAttributeValues::Float32x4(values)) = self.attribute(Mesh::ATTRIBUTE_COLOR) {
            values.clone()
        } else {
            Vec::new()
        };

        MeshBuilder { ids, verts, uvs, colors }.build_mesh(1.)
    }
}

// Add name of mesh here if you want to view the colored vertices
const SKIP_VERTEXCO: [&str; 1] = ["Goal_STD_Trim.pskx"];

/// A collection of inter-connected triangles.
#[derive(Clone, Debug, Default)]
pub struct MeshBuilder {
    ids: Vec<u32>,
    verts: Vec<f32>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
}

impl MeshBuilder {
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

        if !self.colors.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.ids.iter().map(|&id| self.colors[id as usize]).collect::<Vec<_>>());
        }

        mesh
    }

    /// Create a mesh from a Rocket League .pskx file
    pub fn from_pskx(name: &str, bytes: &[u8]) -> Result<Self, bevy::asset::Error> {
        let mut cursor = io::Cursor::new(bytes);

        // ensure file header matches PSK_FILE_HEADER
        let mut file_header = [0; 32];
        cursor.read_exact(&mut file_header)?;
        assert_eq!(&file_header[..PSK_FILE_HEADER.len()], PSK_FILE_HEADER);

        let mut ids = Vec::new();
        let mut verts = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();

        let mut wedges = Vec::new();
        let mut num_materials = 0;
        let mut mat_ids = Vec::new();
        let mut extra_uvs = Vec::new();

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
                }
                "VTXW0000" => {
                    wedges = read_wedges(&chunk_data, chunk_data_count);
                    debug_assert_eq!(wedges.len(), chunk_data_count);
                }
                "FACE0000" => {
                    read_faces(&chunk_data, chunk_data_count, &wedges).into_iter().flatten().for_each(|(id, uv, mat_id)| {
                        ids.push(id);
                        uvs.push(uv);
                        mat_ids.push(mat_id);
                    });
                    debug_assert_eq!(ids.len() / 3, chunk_data_count);
                }
                "MATT0000" => {
                    let materials = read_materials(&chunk_data, chunk_data_count);
                    num_materials = materials.len();
                    println!("{name} uses materials: {materials:?}");
                }
                "VERTEXCO" => {
                    if !SKIP_VERTEXCO.contains(&name) {
                        println!("{name} has unused colored vertices");
                        continue;
                    }

                    colors = read_vertex_colors(&chunk_data, chunk_data_count);
                    debug_assert_eq!(colors.len(), chunk_data_count);
                    debug_assert_eq!(colors.len(), wedges.len());
                }
                "EXTRAUVS" => {
                    extra_uvs.push(read_extra_uvs(&chunk_data, chunk_data_count));
                }
                _ => {
                    println!("Unknown chunk: {chunk_id}");
                }
            }
        }

        if !extra_uvs.is_empty() {
            if uvs.is_empty() {
                debug_assert_eq!(ids.len(), extra_uvs.iter().flatten().count());
                uvs = vec![[0.0, 0.0]; ids.len()];
            }

            let mut last_euv = vec![0; num_materials];
            for (uv, mat_id) in uvs.iter_mut().zip(mat_ids).filter(|(_, mat_id)| *mat_id < extra_uvs.len()) {
                if last_euv[mat_id] < extra_uvs[mat_id].len() {
                    *uv = extra_uvs[mat_id][last_euv[mat_id]];
                    last_euv[mat_id] += 1;
                }
            }
        }

        Ok(Self { ids, verts, uvs, colors })
    }
}
