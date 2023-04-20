use bevy::{prelude::*, render::mesh};
use bevy_mod_picking::PickableBundle;
use serde::Deserialize;
use std::io::{self, Read};

use crate::{
    assets::*,
    camera::EntityName,
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
        .insert((PickableBundle::default(), EntityName::new("ball")))
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

        let node = self.sub_nodes.first()?;

        Some(InfoNode {
            // name: self.name.clone(),
            translation: self.location,
            rotation: self.rotation,
            scale: self.scale,
            static_mesh: node.static_mesh.clone(),
            materials: node.materials.clone(),
            invisitek_materials: node.invisitek_materials.clone(),
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

const BLOCK_MESH_MATS: [&str; 2] = ["CollisionMeshes.Collision_Mat", "FX_General.Mat.CubeMap_HotSpot_Mat"];

#[allow(clippy::too_many_arguments)]
fn load_field(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>, mut state: ResMut<NextState<LoadState>>, asset_server: Res<AssetServer>) {
    let (pickup_boost, standard_common_prefab, the_world): (Section, Node, Node) = serde_json::from_str(include_str!("../stadiums/Stadium_P_MeshObjects.json")).unwrap();
    debug_assert!(pickup_boost.name == "Pickup_Boost");
    debug_assert!(standard_common_prefab.name == "Standard_Common_Prefab");
    debug_assert!(the_world.name == "TheWorld");
    let persistent_level = &the_world.sub_nodes[0];
    debug_assert!(persistent_level.name == "PersistentLevel");

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
            if BLOCK_MESH_MATS.iter().any(|&x| mats.iter().any(|y| y.as_str() == x)) {
                continue;
            }
        }

        let Some(mesh) = get_mesh_info(&node.static_mesh, asset_server.as_ref()) else {
            println!("Not spawning mesh {}", node.static_mesh);
            continue;
        };

        let Some(first_mat) = node.materials.as_ref().and_then(|mats| mats.first()) else {
            println!("No materials found for {}", node.static_mesh);
            continue;
        };

        println!("Getting material(s) for {}...", node.static_mesh);
        let material = get_material(first_mat, materials.as_mut(), asset_server.as_ref());

        let transform = node.get_transform();
        commands
            .spawn(PbrBundle {
                mesh: mesh.clone(),
                material: material.clone(),
                transform,
                ..default()
            })
            .insert((PickableBundle::default(), EntityName::new(node.static_mesh)));
    }

    state.set(LoadState::FieldExtra);
}

// Add name of mesh here if you want to view the colored vertices
const INCLUDE_VERTEXCO: [&str; 1] = ["Goal_STD_Trim.pskx"];

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
                    if !INCLUDE_VERTEXCO.contains(&name) {
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
