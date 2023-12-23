use crate::{
    assets::*,
    bytes::ToBytes,
    camera::{HighlightedEntity, PrimaryCamera},
    rocketsim::{GameMode, GameState},
    udp::{Ball, Car, Connection, ToBevyVec, ToBevyVecFlat},
    LoadState,
};
use bevy::{
    math::{Vec3A, Vec3Swizzles},
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
    render::mesh::{self, VertexAttributeValues},
    time::Stopwatch,
    window::PrimaryWindow,
};
use bevy_eventlistener::callbacks::ListenerInput;
use bevy_mod_picking::{backends::raycast::RaycastPickable, prelude::*};
use include_flate::flate;
use serde::Deserialize;
use std::{
    io::{self, Read},
    rc::Rc,
    str::Utf8Error,
    time::Duration,
};
use thiserror::Error;

#[cfg(debug_assertions)]
use crate::camera::EntityName;

pub struct FieldLoaderPlugin;

impl Plugin for FieldLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LargeBoostPadLocRots::default())
            .insert_resource(StateSetTime::default())
            .add_event::<ChangeBallPos>()
            .add_event::<ChangeCarPos>()
            .add_systems(
                Update,
                (
                    despawn_old_field.run_if(in_state(LoadState::Despawn)),
                    load_field.run_if(in_state(LoadState::Field)),
                    load_extra_field.run_if(in_state(LoadState::FieldExtra)),
                    (
                        advance_stopwatch,
                        (
                            change_ball_pos.run_if(on_event::<ChangeBallPos>()),
                            change_car_pos.run_if(on_event::<ChangeCarPos>()),
                        )
                            .run_if(|last_state_set: Res<StateSetTime>| {
                                // Limit state setting to avoid bogging down the simulation with state setting requests
                                last_state_set.0.elapsed() >= Duration::from_secs_f32(1. / 60.)
                            }),
                    )
                        .chain(),
                ),
            );
    }
}

fn advance_stopwatch(mut last_state_set: ResMut<StateSetTime>, time: Res<Time>) {
    last_state_set.0.tick(time.delta());
}

#[derive(Event)]
pub struct ChangeBallPos;

impl From<ListenerInput<Pointer<Drag>>> for ChangeBallPos {
    fn from(_: ListenerInput<Pointer<Drag>>) -> Self {
        Self
    }
}

#[derive(Resource, Default)]
struct StateSetTime(Stopwatch);

fn change_ball_pos(
    windows: Query<&Window, With<PrimaryWindow>>,
    socket: Res<Connection>,
    mut game_state: ResMut<GameState>,
    mut events: EventReader<ChangeBallPos>,
    camera: Query<(&Camera, &GlobalTransform), With<PrimaryCamera>>,
    mut last_state_set: ResMut<StateSetTime>,
) {
    events.clear();
    let Some(target) = get_move_object_target(camera, windows, game_state.ball.pos.xzy()) else {
        return;
    };

    last_state_set.0.reset();

    game_state.ball.vel = (target.xzy() - game_state.ball.pos).normalize() * 2000.;
    if let Err(e) = socket.0.send(&game_state.to_bytes()) {
        error!("Failed to send ball position: {e}");
    }
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
    socket: Res<Connection>,
    mut game_state: ResMut<GameState>,
    mut events: EventReader<ChangeCarPos>,
    camera: Query<(&Camera, &GlobalTransform), With<PrimaryCamera>>,
    mut last_state_set: ResMut<StateSetTime>,
) {
    let Some(last_event) = events.read().last() else {
        return;
    };

    let Ok(car_id) = cars.get(last_event.0).map(Car::id) else {
        return;
    };

    let Some(car) = game_state.cars.iter_mut().find(|car| car.id == car_id) else {
        return;
    };

    let Some(target) = get_move_object_target(camera, windows, car.state.pos.xzy()) else {
        return;
    };

    last_state_set.0.reset();

    car.state.vel = (target.xzy() - car.state.pos).normalize() * 2000.;
    if let Err(e) = socket.0.send(&game_state.to_bytes()) {
        error!("Failed to send car position: {e}");
    }
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
            #[cfg(debug_assertions)]
            EntityName::from("ball"),
            RaycastPickable,
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

    state.set(LoadState::Field);
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

const BLACKLIST_MESH_MATS: [&str; 8] = [
    "CollisionMeshes.Collision_Mat",
    "Stadium_Assets.Materials.Grass_LOD_Team1_MIC",
    "FutureTech.Materials.Glass_Projected_V2_Team2_MIC",
    "FutureTech.Materials.Glass_Projected_V2_Mat",
    "Trees.Materials.TreeBark_Mat",
    "FutureTech.Materials.TrimLight_None_Mat",
    "City.Materials.Asphalt_Simple_MAT",
    "Graybox_Assets.Materials.NetNonmove_Mat",
];

const NO_SHADOWS: [&str; 1] = ["Proto_BBall.SM.Net_Collision"];

#[derive(Resource, Default)]
pub struct LargeBoostPadLocRots {
    pub locs: Vec<Vec2>,
    pub rots: Vec<f32>,
}

#[derive(Component)]
pub struct StaticFieldEntity;

flate!(pub static STADIUM_P_LAYOUT: str from "stadiums/Stadium_P_MeshObjects.json");
flate!(pub static HOOPS_STADIUM_P_LAYOUT: str from "stadiums/HoopsStadium_P_MeshObjects.json");

fn despawn_old_field(
    mut commands: Commands,
    mut state: ResMut<NextState<LoadState>>,
    static_field_entities: Query<Entity, With<StaticFieldEntity>>,
) {
    static_field_entities.for_each(|entity| {
        commands.entity(entity).despawn();
    });
    state.set(LoadState::Field);
}

fn load_field(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: ResMut<NextState<LoadState>>,
    mut large_boost_pad_loc_rots: ResMut<LargeBoostPadLocRots>,
    game_mode: Res<GameMode>,
    asset_server: Res<AssetServer>,
) {
    let layout: &str = match *game_mode {
        GameMode::TheVoid => {
            state.set(LoadState::None);
            return;
        }
        GameMode::Hoops => &HOOPS_STADIUM_P_LAYOUT,
        _ => &STADIUM_P_LAYOUT,
    };

    let (_pickup_boost, structures, the_world): (Section, Node, Node) = serde_json::from_str(layout).unwrap();
    #[cfg(debug_assertions)]
    {
        // this double-layer of debug_assertion checks is because 'name' won't be present in release mode
        debug_assert_eq!(_pickup_boost.name.as_ref(), "Pickup_Boost");
        debug_assert_eq!(
            structures.name.as_ref(),
            match *game_mode {
                GameMode::Hoops => "Archetypes",
                _ => "Standard_Common_Prefab",
            }
        );
        debug_assert_eq!(the_world.name.as_ref(), "TheWorld");
    }
    let persistent_level = &the_world.sub_nodes[0];
    #[cfg(debug_assertions)]
    debug_assert_eq!(persistent_level.name.as_ref(), "PersistentLevel");

    let all_nodes = structures.sub_nodes[0]
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

        for node in &*obj.sub_nodes {
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

    state.set(LoadState::None);
}

#[derive(Component)]
pub struct SceneType(GameState);

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

    let mats = node.materials.as_ref().map_or_else::<Rc<[Box<str>]>, _, _>(
        || {
            warn!("No materials found for {}", node.static_mesh);
            Rc::from([Box::default()])
        },
        Clone::clone,
    );

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

        let mut obj = commands.spawn((
            PbrBundle {
                mesh,
                material,
                transform,
                ..default()
            },
            #[cfg(debug_assertions)]
            EntityName::from(format!("{} | {mat}", node.static_mesh)),
            RaycastPickable,
            On::<Pointer<Over>>::target_insert(HighlightedEntity),
            On::<Pointer<Out>>::target_remove::<HighlightedEntity>(),
            StaticFieldEntity,
        ));

        if NO_SHADOWS.contains(&node.static_mesh.as_ref()) {
            obj.insert(NotShadowCaster).insert(NotShadowReceiver);
        }
    }
}

// Add name of mesh here if you want to view the colored vertices
const INCLUDE_VERTEXCO: [&str; 2] = ["Goal_STD_Trim", "CrowdSpawnerMesh"];

#[derive(Debug, Error)]
pub enum MeshBuilderError {
    #[error("Invalid file header in pskx file: {0}")]
    FileHeader(#[from] io::Error),
    #[error("Invalid chunk id in pskx file: {0}")]
    ChunkId(#[from] Utf8Error),
}

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
    pub fn from_pskx(name: &str, bytes: &[u8]) -> Result<Self, MeshBuilderError> {
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
            process_materials(&mut uvs, &ids, &extra_uvs, num_materials, &mat_ids);
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

fn process_materials(
    uvs: &mut Vec<[f32; 2]>,
    ids: &Vec<usize>,
    extra_uvs: &[Vec<[f32; 2]>],
    num_materials: usize,
    mat_ids: &[usize],
) {
    if uvs.is_empty() {
        debug_assert_eq!(ids.len(), extra_uvs.iter().flatten().count());
        *uvs = vec![[0.0, 0.0]; ids.len()];
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
