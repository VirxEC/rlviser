use bevy::{
    math::{Vec3A, Vec3Swizzles},
    prelude::*,
    render::mesh::{self, VertexAttributeValues},
    window::PrimaryWindow,
};
use bevy_eventlistener::callbacks::ListenerInput;
use bevy_mod_picking::prelude::*;
use serde::Deserialize;
use std::{
    io::{self, Read},
    rc::Rc,
};

use crate::{
    assets::*,
    bytes::ToBytes,
    camera::{EntityName, HighlightedEntity, PrimaryCamera},
    rocketsim::GameState,
    udp::{Ball, Car, ToBevyVec, ToBevyVecFlat, UdpConnection},
    LoadState,
};

pub struct FieldLoaderPlugin;

impl Plugin for FieldLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LargeBoostPadLocRots::default())
            .add_systems(
                Update,
                (
                    load_field.run_if(in_state(LoadState::Field)),
                    load_extra_field.run_if(in_state(LoadState::FieldExtra)),
                    change_ball_pos.run_if(on_event::<ChangeBallPos>()),
                    change_car_pos.run_if(on_event::<ChangeCarPos>()),
                ),
            )
            .add_event::<ChangeBallPos>()
            .add_event::<ChangeCarPos>();
    }
}

#[derive(Event)]
pub struct ChangeBallPos;

impl From<ListenerInput<Pointer<Drag>>> for ChangeBallPos {
    fn from(_: ListenerInput<Pointer<Drag>>) -> Self {
        Self
    }
}

fn change_ball_pos(
    windows: Query<&Window, With<PrimaryWindow>>,
    socket: Res<UdpConnection>,
    mut game_state: ResMut<GameState>,
    mut events: EventReader<ChangeBallPos>,
    camera: Query<(&Camera, &GlobalTransform), With<PrimaryCamera>>,
) {
    events.clear();
    let Some(target) = get_move_object_target(camera, windows, game_state.ball.pos.xzy()) else {
        return;
    };

    game_state.ball.vel = (target.xzy() - game_state.ball.pos).normalize() * 2000.;
    socket.0.send(&game_state.to_bytes()).unwrap();
}

#[derive(Event)]
pub struct ChangeCarPos(Entity);

impl From<ListenerInput<Pointer<Drag>>> for ChangeCarPos {
    fn from(event: ListenerInput<Pointer<Drag>>) -> Self {
        Self(event.target)
    }
}

fn change_car_pos(
    cars: Query<&Car>,
    windows: Query<&Window, With<PrimaryWindow>>,
    socket: Res<UdpConnection>,
    mut game_state: ResMut<GameState>,
    mut events: EventReader<ChangeCarPos>,
    camera: Query<(&Camera, &GlobalTransform), With<PrimaryCamera>>,
) {
    let Ok(car_id) = cars.get(events.iter().last().unwrap().0).map(|car| car.id()) else {
        return;
    };

    let Some(car) = game_state.cars.iter_mut().find(|car| car.id == car_id) else {
        return;
    };

    let Some(target) = get_move_object_target(camera, windows, car.state.pos.xzy()) else {
        return;
    };

    car.state.vel = (target.xzy() - car.state.pos).normalize() * 2000.;
    socket.0.send(&game_state.to_bytes()).unwrap();
}

fn get_move_object_target(
    camera: Query<(&Camera, &GlobalTransform), With<PrimaryCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    plane_point: Vec3A,
) -> Option<Vec3A> {
    let (camera, global_transform) = camera.single();
    let cursor_coords = windows.single().cursor_position()?;

    // Get the ray that goes from the camera through the cursor
    let global_ray = camera.viewport_to_world(global_transform, cursor_coords)?;

    let cam_pos = Vec3A::from(global_ray.origin);
    let cursor_dir = Vec3A::from(global_ray.direction);

    // define a plane that intersects the ball and is perpendicular to the camera direction
    let plane_normal = (global_transform.affine().matrix3 * Vec3A::Z).normalize();

    // get projection factor
    let lambda = (plane_point - cam_pos).dot(plane_normal) / plane_normal.dot(cursor_dir);

    // project cursor ray onto plane
    Some(cam_pos + lambda * cursor_dir)
}

fn load_extra_field(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<NextState<LoadState>>,
    ball_assets: Res<BallAssets>,
) {
    // load a glowing ball

    let initial_ball_color = Color::rgb(0.3, 0.3, 0.3);

    let ball_material = StandardMaterial {
        base_color_texture: Some(ball_assets.ball_diffuse.clone()),
        // normal_map_texture: Some(ball_assets.ball_normal.clone()),
        // flip_normal_map_y: true,
        // occlusion_texture: Some(ball_assets.ball_occlude.clone()),
        perceptual_roughness: 0.7,
        reflectance: 0.25,
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
            EntityName::from("ball"),
            RaycastPickTarget::default(),
            On::<Pointer<Over>>::target_insert(HighlightedEntity),
            On::<Pointer<Out>>::target_remove::<HighlightedEntity>(),
            On::<Pointer<Drag>>::send_event::<ChangeBallPos>(),
        ))
        .with_children(|parent| {
            parent.spawn(PointLightBundle {
                point_light: PointLight {
                    color: initial_ball_color,
                    radius: 90.,
                    intensity: 2_000_000.,
                    range: 1000.,
                    ..default()
                },
                ..default()
            });
        });

    state.set(LoadState::Connect);
}

fn rc_string_default() -> Rc<str> {
    Rc::from("")
}

#[derive(Debug, Deserialize)]
struct InfoNode {
    // name: String,
    #[serde(rename = "Translation")]
    translation: Option<[f32; 3]>,
    #[serde(rename = "Rotation")]
    rotation: Option<[f32; 3]>,
    #[serde(rename = "Scale")]
    scale: Option<[f32; 3]>,
    #[serde(rename = "StaticMesh", default = "rc_string_default")]
    static_mesh: Rc<str>,
    #[serde(rename = "Materials")]
    materials: Option<Rc<[Box<str>]>>,
    #[serde(rename = "InvisiTekMaterials")]
    invisitek_materials: Option<Rc<[Box<str>]>>,
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
            scale: self.scale.map_or(Vec3::ONE, ToBevyVec::to_bevy),
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
    sub_nodes: Rc<[InfoNode]>,
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
    #[cfg(debug_assertions)]
    name: Box<str>,
    #[serde(rename = "subNodes")]
    sub_nodes: Box<[ObjectNode]>,
}

#[derive(Debug, Deserialize)]
struct Node {
    #[cfg(debug_assertions)]
    name: Box<str>,
    #[serde(rename = "subNodes")]
    sub_nodes: Box<[Section]>,
}

const BLACKLIST_MESH_MATS: [&str; 6] = [
    "CollisionMeshes.Collision_Mat",
    "Stadium_Assets.Materials.Grass_LOD_Team1_MIC",
    "FutureTech.Materials.Glass_Projected_V2_Team2_MIC",
    "FutureTech.Materials.Glass_Projected_V2_Mat",
    "Trees.Materials.TreeBark_Mat",
    "FutureTech.Materials.TrimLight_None_Mat",
];

#[derive(Resource, Default)]
pub struct LargeBoostPadLocRots {
    pub locs: Vec<Vec2>,
    pub rots: Vec<f32>,
}

fn load_field(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: ResMut<NextState<LoadState>>,
    mut large_boost_pad_loc_rots: ResMut<LargeBoostPadLocRots>,
    asset_server: Res<AssetServer>,
) {
    let (_pickup_boost, standard_common_prefab, the_world): (Section, Node, Node) =
        serde_json::from_str(include_str!("../stadiums/Stadium_P_MeshObjects.json")).unwrap();
    #[cfg(debug_assertions)]
    {
        // this double-layer of debug_assertion checks is because 'name' won't be present in release mode
        debug_assert!(_pickup_boost.name.as_ref() == "Pickup_Boost");
        debug_assert!(standard_common_prefab.name.as_ref() == "Standard_Common_Prefab");
        debug_assert!(the_world.name.as_ref() == "TheWorld");
    }
    let persistent_level = &the_world.sub_nodes[0];
    #[cfg(debug_assertions)]
    debug_assert!(persistent_level.name.as_ref() == "PersistentLevel");

    let all_nodes = standard_common_prefab.sub_nodes[0]
        .sub_nodes
        .iter()
        .chain(persistent_level.sub_nodes.iter());

    for obj in all_nodes {
        if let Some(node) = obj.get_info_node() {
            process_info_node(
                &node,
                &asset_server,
                &mut meshes,
                &mut materials,
                &mut large_boost_pad_loc_rots,
                &mut commands,
            );
            continue;
        }

        for node in obj.sub_nodes.iter() {
            process_info_node(
                node,
                &asset_server,
                &mut meshes,
                &mut materials,
                &mut large_boost_pad_loc_rots,
                &mut commands,
            );
        }
    }

    state.set(LoadState::FieldExtra);
}

fn process_info_node(
    node: &InfoNode,
    asset_server: &AssetServer,
    meshes: &mut ResMut<'_, Assets<Mesh>>,
    materials: &mut Assets<StandardMaterial>,
    large_boost_pad_loc_rots: &mut LargeBoostPadLocRots,
    commands: &mut Commands,
) {
    if node.static_mesh.trim().is_empty() {
        return;
    }

    let Some(mesh) = get_mesh_info(&node.static_mesh, meshes.as_mut()) else {
        return;
    };

    let mats = if let Some(mats) = node.materials.as_ref() {
        mats.clone()
    } else {
        warn!("No materials found for {}", node.static_mesh);
        Rc::from([Box::from("")])
    };

    debug!("Spawning {}", node.static_mesh);

    for (mesh, mat) in mesh.into_iter().zip(mats.iter()) {
        if BLACKLIST_MESH_MATS.contains(&mat.as_ref()) {
            continue;
        }

        let material = get_material(mat, materials, asset_server, None);

        let mut transform = node.get_transform();

        if node.static_mesh.contains("Grass.Grass") || node.static_mesh.contains("Grass_1x1") {
            transform.translation.y += 10.;
        } else if node.static_mesh.contains("BoostPad_Large") {
            large_boost_pad_loc_rots
                .locs
                .push(node.translation.map(ToBevyVecFlat::to_bevy_flat).unwrap_or_default());
            large_boost_pad_loc_rots
                .rots
                .push(node.rotation.map(|r| r[1]).unwrap_or_default());
        }

        commands.spawn((
            PbrBundle {
                mesh,
                material,
                transform,
                ..default()
            },
            EntityName::from(format!("{} | {mat}", node.static_mesh)),
            RaycastPickTarget::default(),
            On::<Pointer<Over>>::target_insert(HighlightedEntity),
            On::<Pointer<Out>>::target_remove::<HighlightedEntity>(),
        ));
    }
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
                    .filter_map(|(mat_ids, verts)| {
                        if mat_ids[0] == mat_id {
                            Some([verts[0], verts[1], verts[2]])
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .collect::<Vec<_>>();
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts);

                let uvs = all_mat_ids
                    .chunks_exact(3)
                    .zip(all_uvs.chunks_exact(3))
                    .filter_map(|(mat_ids, uvs)| {
                        if mat_ids[0] == mat_id {
                            Some([uvs[0], uvs[1], uvs[2]])
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .collect::<Vec<_>>();
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

                let normals = all_mat_ids
                    .chunks_exact(3)
                    .zip(all_normals.chunks_exact(3))
                    .filter_map(|(mat_ids, normals)| {
                        if mat_ids[0] == mat_id {
                            Some([normals[0], normals[1], normals[2]])
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .collect::<Vec<_>>();
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

                let tangents = all_mat_ids
                    .chunks_exact(3)
                    .zip(all_tangents.chunks_exact(3))
                    .filter_map(|(mat_ids, tangents)| {
                        if mat_ids[0] == mat_id {
                            Some([tangents[0], tangents[1], tangents[2]])
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .collect::<Vec<_>>();
                mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);

                if let Some(all_colors) = all_colors {
                    let colors = all_mat_ids
                        .chunks_exact(3)
                        .zip(all_colors.chunks_exact(3))
                        .filter_map(|(mat_ids, colors)| {
                            if mat_ids[0] == mat_id {
                                Some([colors[0], colors[1], colors[2]])
                            } else {
                                None
                            }
                        })
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
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_COLOR,
                self.ids.iter().map(|&id| self.colors[id]).collect::<Vec<_>>(),
            );
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
            let chunk_data_size =
                i32::from_le_bytes([chunk_header[24], chunk_header[25], chunk_header[26], chunk_header[27]]) as usize;
            let chunk_data_count =
                i32::from_le_bytes([chunk_header[28], chunk_header[29], chunk_header[30], chunk_header[31]]) as usize;

            if chunk_data_count == 0 {
                continue;
            }

            let mut chunk_data = vec![0; chunk_data_size * chunk_data_count];
            cursor.read_exact(&mut chunk_data)?;

            // use plenty of debug asserts to ensure valid data processing
            // with no performance impact in release mode
            match chunk_id {
                "PNTS0000" => {
                    read_vertices(&chunk_data, chunk_data_count, &mut verts);
                    debug_assert_eq!(verts.len() / 3, chunk_data_count);
                    debug_assert_eq!(verts.len() % 3, 0);
                }
                "VTXW0000" => {
                    read_wedges(&chunk_data, chunk_data_count, &mut wedges);
                    debug_assert_eq!(wedges.len(), chunk_data_count);
                }
                "FACE0000" => {
                    read_faces(&chunk_data, chunk_data_count, &wedges, &mut ids, &mut uvs, &mut mat_ids);
                    debug_assert_eq!(ids.len() / 3, chunk_data_count);
                }
                "MATT0000" => {
                    let materials = read_materials(&chunk_data, chunk_data_count);
                    num_materials = materials.len();
                }
                "VERTEXCO" => {
                    if !INCLUDE_VERTEXCO.iter().any(|&part| name.contains(part)) {
                        if cfg!(debug_assertions) {
                            warn!("{name} has unused colored vertices");
                        }
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
                    if cfg!(debug_assertions) {
                        error!("Unknown chunk: {chunk_id}");
                    }
                }
            }
        }

        if !extra_uvs.is_empty() {
            if uvs.is_empty() {
                debug_assert_eq!(ids.len(), extra_uvs.iter().flatten().count());
                uvs = vec![[0.0, 0.0]; ids.len()];
            }

            let mut last_euv = vec![0; num_materials];
            for (uv, mat_id) in uvs
                .iter_mut()
                .zip(mat_ids.iter().copied())
                .filter(|(_, mat_id)| *mat_id < extra_uvs.len())
            {
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
