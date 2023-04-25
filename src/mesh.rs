use bevy::{
    prelude::*,
    render::mesh::{self, VertexAttributeValues},
};
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

fn load_extra_field(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<NextState<LoadState>>,
    ball_assets: Res<BallAssets>,
) {
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

    // spawn stadium lights

    commands
        .spawn(PbrBundle {
            mesh: meshes.add(shape::UVSphere { radius: 250., ..default() }.into()),
            material: materials.add(Color::rgb(1., 0., 0.).into()),
            transform: Transform::from_xyz(-11500., 9000., 11500.),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn(PbrBundle {
                mesh: meshes.add(shape::UVSphere { radius: 30., ..default() }.into()),
                material: materials.add(Color::rgb(0., 0., 1.).into()),
                ..default()
            });
        })
        .insert((PickableBundle::default(), EntityName::new("generic_ball")));

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

const BLOCK_MESH_MATS: [&str; 6] = [
    "CollisionMeshes.Collision_Mat",
    "Stadium_Assets.Materials.Grass_LOD_Team1_MIC",
    "FutureTech.Materials.Glass_Projected_V2_Team2_MIC",
    "FutureTech.Materials.Glass_Projected_V2_Mat",
    "Trees.Materials.TreeBark_Mat",
    "FutureTech.Materials.TrimLight_None_Mat",
];

fn load_field(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: ResMut<NextState<LoadState>>,
    asset_server: Res<AssetServer>,
) {
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

    let default_mats = vec![String::new()];

    for node in world_nodes.chain(prefab_nodes) {
        if node.static_mesh.trim().is_empty() {
            continue;
        }

        let Some(mesh) = get_mesh_info(&node.static_mesh, meshes.as_mut()) else {
            continue;
        };

        let mats = match node.materials.as_ref() {
            Some(mats) => mats,
            None => {
                warn!("No materials found for {}", node.static_mesh);
                &default_mats
            }
        };

        info!("Spawning {}", node.static_mesh);
        for (mesh, mat) in mesh.into_iter().zip(mats) {
            if BLOCK_MESH_MATS.contains(&mat.as_str()) {
                continue;
            }

            let material = get_material(mat, materials.as_mut(), asset_server.as_ref(), None);

            let mut transform = node.get_transform();

            if node.static_mesh.contains("Grass.Grass") || node.static_mesh.contains("Grass_1x1") {
                transform.translation.y += 10.;
            }

            commands
                .spawn(PbrBundle {
                    mesh: mesh.clone(),
                    material: material.clone(),
                    transform,
                    ..default()
                })
                .insert((PickableBundle::default(), EntityName::new(format!("{} | {mat}", node.static_mesh.clone()))));
        }
    }

    state.set(LoadState::FieldExtra);
}

// Add name of mesh here if you want to view the colored vertices
const INCLUDE_VERTEXCO: [&str; 2] = ["Goal_STD_Trim", "CrowdSpawnerMesh"];

/// A collection of inter-connected triangles.
#[derive(Clone, Debug, Default)]
pub struct MeshBuilder {
    ids: Vec<usize>,
    verts: Vec<f32>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    num_materials: usize,
    mat_ids: Vec<usize>,
}

impl MeshBuilder {
    #[must_use]
    // Build the Bevy Mesh
    pub fn build_meshes(self, scale: f32) -> Vec<Mesh> {
        let num_materials = self.num_materials;

        if num_materials < 2 {
            return vec![self.build_mesh(scale)];
        }

        let all_mat_ids = self.ids.iter().map(|&id| self.mat_ids[id]).collect::<Vec<_>>();

        let initial_mesh = self.build_mesh(scale);
        let all_verts = initial_mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        let VertexAttributeValues::Float32x2(all_uvs) = initial_mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap() else {
            panic!("No UVs found");
        };
        let all_normals = initial_mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap().as_float3().unwrap();
        let VertexAttributeValues::Float32x4(all_tangents) = initial_mesh.attribute(Mesh::ATTRIBUTE_TANGENT).unwrap() else {
            panic!("No tangents found");
        };
        let all_colors = initial_mesh.attribute(Mesh::ATTRIBUTE_COLOR).map(|colors| match colors {
            VertexAttributeValues::Float32x4(colors) => colors,
            _ => panic!("No colors found"),
        });

        (0..num_materials)
            .map(|mat_id| {
                let mut mesh = Mesh::new(mesh::PrimitiveTopology::TriangleList);

                let verts = all_mat_ids
                    .chunks_exact(3)
                    .zip(all_verts.chunks_exact(3))
                    .filter_map(|(mat_ids, verts)| if mat_ids[0] == mat_id { Some([verts[0], verts[1], verts[2]]) } else { None })
                    .flatten()
                    .collect::<Vec<_>>();
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts);

                let uvs = all_mat_ids
                    .chunks_exact(3)
                    .zip(all_uvs.chunks_exact(3))
                    .filter_map(|(mat_ids, uvs)| if mat_ids[0] == mat_id { Some([uvs[0], uvs[1], uvs[2]]) } else { None })
                    .flatten()
                    .collect::<Vec<_>>();
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

                let normals = all_mat_ids
                    .chunks_exact(3)
                    .zip(all_normals.chunks_exact(3))
                    .filter_map(|(mat_ids, normals)| if mat_ids[0] == mat_id { Some([normals[0], normals[1], normals[2]]) } else { None })
                    .flatten()
                    .collect::<Vec<_>>();
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

                let tangents = all_mat_ids
                    .chunks_exact(3)
                    .zip(all_tangents.chunks_exact(3))
                    .filter_map(
                        |(mat_ids, tangents)| {
                            if mat_ids[0] == mat_id {
                                Some([tangents[0], tangents[1], tangents[2]])
                            } else {
                                None
                            }
                        },
                    )
                    .flatten()
                    .collect::<Vec<_>>();
                mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);

                if let Some(all_colors) = all_colors {
                    let colors = all_mat_ids
                        .chunks_exact(3)
                        .zip(all_colors.chunks_exact(3))
                        .filter_map(|(mat_ids, colors)| if mat_ids[0] == mat_id { Some([colors[0], colors[1], colors[2]]) } else { None })
                        .flatten()
                        .collect::<Vec<_>>();
                    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
                }

                mesh
            })
            .collect()
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

        let ids = self.ids.iter().map(|&id| verts[id]).collect::<Vec<_>>();
        let num_verts = ids.len();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, ids);

        mesh.compute_flat_normals();

        if !self.uvs.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);

            // compute tangents
            mesh.set_indices(Some(mesh::Indices::U32((0..num_verts as u32).collect::<Vec<_>>())));
            mesh.generate_tangents().unwrap();
        }

        if !self.colors.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.ids.iter().map(|&id| self.colors[id]).collect::<Vec<_>>());
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
                        ids.push(id as usize);
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
                    if !INCLUDE_VERTEXCO.iter().any(|&part| name.contains(part)) {
                        warn!("{name} has unused colored vertices");
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
                    error!("Unknown chunk: {chunk_id}");
                }
            }
        }

        if !extra_uvs.is_empty() {
            if uvs.is_empty() {
                debug_assert_eq!(ids.len(), extra_uvs.iter().flatten().count());
                uvs = vec![[0.0, 0.0]; ids.len()];
            }

            let mut last_euv = vec![0; num_materials];
            for (uv, mat_id) in uvs.iter_mut().zip(mat_ids.iter().copied()).filter(|(_, mat_id)| *mat_id < extra_uvs.len()) {
                if last_euv[mat_id] < extra_uvs[mat_id].len() {
                    *uv = extra_uvs[mat_id][last_euv[mat_id]];
                    last_euv[mat_id] += 1;
                }
            }
        }

        Ok(Self {
            ids,
            verts,
            uvs,
            colors,
            num_materials,
            mat_ids,
        })
    }
}
