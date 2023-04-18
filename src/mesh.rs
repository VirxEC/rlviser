use bevy::{prelude::*, render::mesh};
use serde::Deserialize;
use std::io::{self, Read};

use crate::{
    assets::*,
    udp::{Ball, ToBevyVec},
    LoadState,
};

pub struct FieldLoaderPlugin;

impl Plugin for FieldLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(load_field.run_if(in_state(LoadState::Field)))
            .add_system(load_extra_field.run_if(in_state(LoadState::FieldExtra)));
    }
}

fn load_extra_field(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>, mut state: ResMut<NextState<LoadState>>, ball_assets: Res<BallAssets>) {
    // load a glowing ball

    let initial_ball_color = Color::rgb(0.3, 0.3, 0.3);

    let ball_material = StandardMaterial {
        base_color_texture: Some(ball_assets.ball_diffuse.clone()),
        unlit: true,
        // normal_map_texture: Some(ball_assets.ball_normal.clone()),
        // occlusion_texture: Some(ball_assets.ball_occlude.clone()),
        // perceptual_roughness: 0.4,
        // metallic: 0.,
        ..default()
    };

    commands
        .spawn((
            Ball,
            PbrBundle {
                mesh: ball_assets.ball.clone(),
                material: materials.add(ball_material),
                transform: Transform::from_xyz(0., 92., 0.),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn(PointLightBundle {
                point_light: PointLight {
                    color: initial_ball_color,
                    radius: 90.,
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

#[derive(Clone, Debug, Deserialize)]
struct InfoNode {
    // name: String,
    #[serde(rename = "Translation")]
    translation: Option<[f32; 3]>,
    #[serde(rename = "Rotation")]
    rotation: Option<[f32; 3]>,
    #[serde(rename = "Scale")]
    scale: Option<[f32; 3]>,
    #[serde(rename = "StaticMesh", default)]
    static_mesh: String,
    #[serde(rename = "Materials")]
    materials: Option<Vec<String>>,
    #[serde(rename = "InvisiTekMaterials")]
    invisitek_materials: Option<Vec<String>>,
}

impl InfoNode {
    #[inline]
    fn get_transform(&self) -> Transform {
        Transform {
            translation: self.translation.unwrap_or_default().to_bevy(),
            rotation: {
                let [a, b, c] = self.rotation.unwrap_or_default();
                Quat::from_euler(EulerRot::ZYX, c.to_radians(), b.to_radians(), a.to_radians())
            },
            scale: self.scale.map(ToBevyVec::to_bevy).unwrap_or(Vec3::ONE),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ObjectNode {
    // name: String,
    #[serde(rename = "Location")]
    location: Option<[f32; 3]>,
    #[serde(rename = "Rotation")]
    rotation: Option<[f32; 3]>,
    #[serde(rename = "Scale")]
    scale: Option<[f32; 3]>,
    #[serde(rename = "subNodes")]
    sub_nodes: Vec<InfoNode>,
}

impl ObjectNode {
    #[inline]
    fn get_info_node(&self) -> Option<InfoNode> {
        if self.location.is_none() && self.rotation.is_none() && self.scale.is_none() {
            return None;
        }

        let (materials, invisitek_materials) = self.sub_nodes.first().map(|node| (node.materials.clone(), node.invisitek_materials.clone()))?;

        Some(InfoNode {
            // name: self.name.clone(),
            translation: self.location,
            rotation: self.rotation,
            scale: self.scale,
            static_mesh: String::new(),
            materials,
            invisitek_materials,
        })
    }
}

#[derive(Debug, Deserialize)]
struct Section {
    name: String,
    #[serde(rename = "subNodes")]
    sub_nodes: Vec<ObjectNode>,
}

#[derive(Debug, Deserialize)]
struct Node {
    name: String,
    #[serde(rename = "subNodes")]
    sub_nodes: Vec<Section>,
}

#[allow(clippy::too_many_arguments)]
fn load_field(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<NextState<LoadState>>,
    tiled_patterns: Res<TiledPatterns>,
    detail_normals: Res<Details>,
    park_stadium: Res<ParkStadium>,
    future_stadium: Res<FutureStadium>,
    fx_textures: Res<FxTextures>,
) {
    let detail_normals = detail_normals.as_ref();
    let tiled_patterns = tiled_patterns.as_ref();
    let park_stadium = park_stadium.as_ref();
    let future_stadium = future_stadium.as_ref();
    let fx_textures = fx_textures.as_ref();

    let (pickup_boost, standard_common_prefab, the_world): (Section, Node, Node) = serde_json::from_str(include_str!("../stadiums/Stadium_P_MeshObjects.json")).unwrap();
    debug_assert!(pickup_boost.name == "Pickup_Boost");
    debug_assert!(standard_common_prefab.name == "Standard_Common_Prefab");
    debug_assert!(the_world.name == "TheWorld");
    let persistent_level = &the_world.sub_nodes[0];
    debug_assert!(persistent_level.name == "PersistentLevel");

    dbg!(&standard_common_prefab.sub_nodes[0]);
    let prefab_nodes = standard_common_prefab.sub_nodes[0].sub_nodes.iter().flat_map(|node| node.get_info_node());
    let world_nodes = persistent_level.sub_nodes.iter().flat_map(|node| match node.get_info_node() {
        Some(node) => vec![node],
        None => node.sub_nodes.clone(),
    });

    for node in world_nodes.chain(prefab_nodes) {
        if node.static_mesh.trim().is_empty() {
            continue;
        }

        if let Some(mats) = &node.materials {
            if mats.contains(&String::from("CollisionMeshes.Collision_Mat")) {
                continue;
            }
        }

        if let Some(mesh) = get_mesh_info(&node.static_mesh, &[park_stadium, future_stadium]) {
            println!("Getting material for {}...", node.static_mesh);
            let material = get_material(
                &node.materials.as_ref().unwrap()[0],
                &mut materials,
                &[fx_textures, tiled_patterns, detail_normals, future_stadium],
            );

            let transform = node.get_transform();
            commands.spawn(PbrBundle {
                mesh: mesh.clone(),
                material: material.clone(),
                transform,
                ..default()
            });
        } else {
            println!("No mesh for {}", node.static_mesh);
        }
    }

    state.set(LoadState::FieldExtra);
}

// fn load_field(
//     mut commands: Commands,
//     mut meshes: ResMut<Assets<Mesh>>,
//     mut materials: ResMut<Assets<StandardMaterial>>,
//     mut state: ResMut<NextState<LoadState>>,
//     tiled_patterns: Res<TiledPatterns>,
//     detail_normals: Res<Details>,
//     future_stadium: Res<FutureStadium>,
// ) {
//     commands.spawn(PbrBundle {
//         mesh: future_stadium.oob_floor.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.3, 0.3, 0.3),
//             base_color_texture: Some(tiled_patterns.hexagons_pack_b.clone()),
//             normal_map_texture: Some(tiled_patterns.hexagons_normal.clone()),
//             metallic: 0.1,
//             ..default()
//         }),
//         ..default()
//     });

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.oob_floor_trim.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.3, 0.3, 0.3),
//             normal_map_texture: Some(detail_normals.brushed_metal_normal.clone()),
//             emissive: Color::rgb(0.1, 0.1, 0.1),
//             metallic: 0.8,
//             ..default()
//         }),
//         ..default()
//     });

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.field_center.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.9, 0.3, 0.3),
//             metallic: 0.1,
//             ..default()
//         }),
//         ..default()
//     });

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.field_center_lines.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.9, 0.3, 0.3),
//             metallic: 0.1,
//             ..default()
//         }),
//         ..default()
//     });

//     // commands.spawn(PbrBundle {
//     //     mesh: dfh_stadium.field_center_trim.clone(),
//     //     material: materials.add(StandardMaterial {
//     //         base_color: Color::rgb(0.9, 0.3, 0.3),
//     //         metallic: 0.1,
//     //         cull_mode: None,
//     //         double_sided: true,
//     //         ..default()
//     //     }),
//     //     ..default()
//     // });

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.field_center_field_team1.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.9, 0.3, 0.3),
//             metallic: 0.1,
//             ..default()
//         }),
//         ..default()
//     });

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.field_center_field_team2.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.9, 0.3, 0.3),
//             metallic: 0.1,
//             ..default()
//         }),
//         ..default()
//     });

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.field_center_vent.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.9, 0.3, 0.3),
//             metallic: 0.1,
//             alpha_mode: AlphaMode::Blend,
//             ..default()
//         }),
//         ..default()
//     });

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.field_std_floor_team1.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.3, 0.9, 0.3),
//             perceptual_roughness: 0.9,
//             reflectance: 0.1,
//             ..default()
//         }),
//         ..default()
//     });

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.field_std_floor_team2.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.3, 0.9, 0.3),
//             perceptual_roughness: 0.9,
//             reflectance: 0.1,
//             ..default()
//         }),
//         ..default()
//     });

//     let ff_cage_full = meshes.get(&future_stadium.ff_cage_full.clone()).unwrap().flip();
//     let inv_ff_cage_full = meshes.add(ff_cage_full);
//     let ff_cage_full_mat = materials.add(StandardMaterial {
//         base_color: Color::rgb(0.3, 0.3, 0.3),
//         metallic: 0.1,
//         cull_mode: None,
//         double_sided: true,
//         ..default()
//     });

//     let ff_goal = meshes.get(&future_stadium.ff_goal.clone()).unwrap().flip();
//     let inv_ff_goal = meshes.add(ff_goal);

//     let ff_roof = meshes.get(&future_stadium.ff_roof.clone()).unwrap().flip();
//     let inv_ff_roof = meshes.add(ff_roof);

//     let ff_side = meshes.get(&future_stadium.ff_side.clone()).unwrap().flip();
//     let inv_ff_side = meshes.add(ff_side);

//     let glass_mat = materials.add(StandardMaterial {
//         base_color: Color::rgb(0.3, 0.3, 0.3),
//         base_color_texture: Some(tiled_patterns.hexagons_pack.clone()),
//         metallic: 0.1,
//         cull_mode: None,
//         double_sided: true,
//         alpha_mode: AlphaMode::Blend,
//         ..default()
//     });

//     let goal_lines = meshes.get(&future_stadium.goal_lines.clone()).unwrap().flip();
//     let inv_goal_lines = meshes.add(goal_lines);
//     let goal_lines_mat = materials.add(StandardMaterial {
//         base_color: Color::rgb(0.3, 0.3, 0.9),
//         metallic: 0.1,
//         ..default()
//     });

//     let goal_glass_mat = materials.add(StandardMaterial {
//         base_color: Color::rgba(0.3, 0.3, 0.3, 0.5),
//         metallic: 0.1,
//         cull_mode: None,
//         double_sided: true,
//         alpha_mode: AlphaMode::Blend,
//         ..default()
//     });

//     let field_frame_outer = meshes.get(&future_stadium.field_frame_outer.clone()).unwrap().flip();
//     let inv_field_frame_outer = meshes.add(field_frame_outer);
//     let field_frame_outer_mat = materials.add(StandardMaterial {
//         base_color: Color::rgb(0.3, 0.3, 0.3),
//         metallic: 0.1,
//         cull_mode: None,
//         double_sided: true,
//         ..default()
//     });

//     let goal_std_trim_mat = materials.add(StandardMaterial {
//         base_color: Color::rgb(0.3, 0.3, 0.3),
//         metallic: 0.1,
//         ..default()
//     });

//     let field_std_frame_mat = materials.add(StandardMaterial {
//         base_color: Color::rgb(0.3, 0.3, 0.3),
//         metallic: 0.1,
//         ..default()
//     });

//     let goal_std_floor_mat = materials.add(StandardMaterial {
//         base_color: Color::rgb(0.9, 0.3, 0.3),
//         metallic: 0.1,
//         ..default()
//     });

//     let field_std_trim_mat = materials.add(StandardMaterial {
//         base_color: Color::rgb(0.9, 0.3, 0.3),
//         metallic: 0.1,
//         ..default()
//     });

//     let field_std_trim_b_mat = materials.add(StandardMaterial {
//         base_color: Color::rgb(0.3, 0.3, 0.3),
//         normal_map_texture: Some(detail_normals.brushed_metal_normal.clone()),
//         emissive: Color::rgb(0.1, 0.1, 0.1),
//         metallic: 0.8,
//         ..default()
//     });

//     let side_trim = meshes.get(&future_stadium.side_trim.clone()).unwrap().flip();
//     let inv_side_trim = meshes.add(side_trim);
//     let side_trim_mat = materials.add(StandardMaterial {
//         base_color: Color::rgb(0.3, 0.9, 0.3),
//         metallic: 0.1,
//         cull_mode: None,
//         double_sided: true,
//         ..default()
//     });

//     let field_side_lines = meshes.get(&future_stadium.field_side_lines.clone()).unwrap().flip();
//     let inv_field_side_lines = meshes.add(field_side_lines);
//     let field_side_lines_mat = materials.add(StandardMaterial {
//         base_color: Color::rgb(0.9, 0.3, 0.3),
//         metallic: 0.1,
//         ..default()
//     });

//     let mut stadium_transform = Transform::default();
//     for i in (-1..=1).step_by(2) {
//         commands.spawn(PbrBundle {
//             mesh: future_stadium.field_side_lines.clone(),
//             material: field_side_lines_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: inv_field_side_lines.clone(),
//             material: field_side_lines_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.side_trim.clone(),
//             material: side_trim_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: inv_side_trim.clone(),
//             material: side_trim_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.field_frame_outer.clone(),
//             material: field_frame_outer_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: inv_field_frame_outer.clone(),
//             material: field_frame_outer_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.goal_lines.clone(),
//             material: goal_lines_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: inv_goal_lines.clone(),
//             material: goal_lines_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.field_std_frame.clone(),
//             material: field_std_frame_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.ff_cage_full.clone(),
//             material: ff_cage_full_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: inv_ff_cage_full.clone(),
//             material: ff_cage_full_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.ff_goal.clone(),
//             material: glass_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: inv_ff_goal.clone(),
//             material: glass_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.ff_roof.clone(),
//             material: glass_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: inv_ff_roof.clone(),
//             material: glass_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.ff_side.clone(),
//             material: glass_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: inv_ff_side.clone(),
//             material: glass_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         let goal_transform = Transform::from_xyz(0., 0., -5120. * i as f32).with_rotation(stadium_transform.rotation);

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.goal_std_trim.clone(),
//             material: goal_std_trim_mat.clone(),
//             transform: goal_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.goal_std_frame.clone(),
//             material: goal_glass_mat.clone(),
//             transform: goal_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.goal_std_quarterpipe.clone(),
//             material: goal_std_trim_mat.clone(),
//             transform: goal_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.goal_std_glass.clone(),
//             material: goal_glass_mat.clone(),
//             transform: goal_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.goal_std_floor.clone(),
//             material: goal_std_floor_mat.clone(),
//             transform: goal_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.field_std_trim.clone(),
//             material: field_std_trim_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         commands.spawn(PbrBundle {
//             mesh: future_stadium.field_std_trim_b.clone(),
//             material: field_std_trim_b_mat.clone(),
//             transform: stadium_transform,
//             ..default()
//         });

//         stadium_transform.rotate_local_y(PI);
//     }

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.field_std_floor_hex.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0., 0.3, 0.9),
//             metallic: 0.1,
//             ..default()
//         }),
//         ..default()
//     });

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.boost_pads_01_combined.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.3, 0.9, 0.3),
//             metallic: 0.1,
//             ..default()
//         }),
//         transform: Transform::from_xyz(0., 0., -3250.),
//         ..default()
//     });

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.boost_pads_02_combined.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.3, 0.9, 0.3),
//             metallic: 0.1,
//             ..default()
//         }),
//         ..default()
//     });

//     commands.spawn(PbrBundle {
//         mesh: future_stadium.boost_pads_03_combined.clone(),
//         material: materials.add(StandardMaterial {
//             base_color: Color::rgb(0.3, 0.9, 0.3),
//             metallic: 0.1,
//             ..default()
//         }),
//         transform: Transform::from_xyz(0., 0., 3250.),
//         ..default()
//     });

//     state.set(LoadState::FieldExtra);
// }

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

            // if name == "OOBFloor.pskx" {
            //     // save uvs to json in current directory
            //     let mut file = std::fs::File::create("uvs.csv").unwrap();
            //     writeln!(file, "u, v").unwrap();

            //     for uv in &uvs {
            //         writeln!(file, "{}, {}", uv[0], uv[1]).unwrap();
            //     }
            // }
        }

        Ok(Self { ids, verts, uvs, colors })
    }
}
