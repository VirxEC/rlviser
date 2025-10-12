use crate::{
    GameLoadState,
    assets::*,
    rocketsim::{GameMode, Team},
    settings::state_setting::{EnableTileInfo, UserTileStates},
    udp::{Ball, Tile, ToBevyVec, ToBevyVecFlat, get_tile_color, target_insert, target_remove, write_message},
};
use bevy::{
    asset::{LoadState, RenderAssetUsages},
    color::palettes::css,
    light::{NotShadowCaster, NotShadowReceiver},
    math::Vec3A,
    mesh,
    picking::mesh_picking::ray_cast::SimplifiedMesh,
    prelude::*,
    render::renderer::RenderDevice,
    time::Stopwatch,
    window::PrimaryWindow,
};
use include_flate::flate;
use serde::Deserialize;
use std::{
    cmp::Ordering,
    fs::{File, create_dir_all},
    io::{self, Read},
    path::Path,
    rc::Rc,
    str::Utf8Error,
};
use thiserror::Error;

use crate::{
    camera::{HighlightedEntity, PrimaryCamera},
    settings::state_setting::{EnableBallInfo, EnableCarInfo, EnablePadInfo, UserCarStates, UserPadStates},
    udp::{BoostPadI, Car, Connection, GameStates, SendableUdp},
};
use std::time::Duration;

#[cfg(feature = "team_goal_barriers")]
use crate::udp::{BLUE_COLOR, ORANGE_COLOR};

#[cfg(debug_assertions)]
use crate::camera::EntityName;

pub struct FieldLoaderPlugin;

impl Plugin for FieldLoaderPlugin {
    fn build(&self, app: &mut App) {
        {
            app.add_message::<ChangeBallPos>()
                .add_message::<ChangeCarPos>()
                .add_message::<BallClicked>()
                .add_message::<CarClicked>()
                .add_message::<TileClicked>()
                .add_message::<BoostPadClicked>()
                .insert_resource(StateSetTime::default())
                .add_systems(
                    Update,
                    (
                        handle_ball_clicked.run_if(on_message::<BallClicked>),
                        handle_car_clicked.run_if(on_message::<CarClicked>),
                        handle_boost_pad_clicked.run_if(on_message::<BoostPadClicked>),
                        handle_tile_clicked.run_if(on_message::<TileClicked>),
                        (
                            advance_stopwatch,
                            (
                                change_ball_pos.run_if(on_message::<ChangeBallPos>),
                                change_car_pos.run_if(on_message::<ChangeCarPos>),
                            )
                                .run_if(|last_state_set: Res<StateSetTime>| {
                                    // Limit state setting to avoid bogging down the simulation with state setting requests
                                    last_state_set.0.elapsed() >= Duration::from_secs_f32(1. / 30.)
                                }),
                        )
                            .chain(),
                    ),
                );
        }

        app.insert_resource(LargeBoostPadLocRots::default()).add_systems(
            Update,
            (
                despawn_old_field.run_if(in_state(GameLoadState::Despawn)),
                load_field.run_if(in_state(GameLoadState::Field)),
                load_extra_field.run_if(in_state(GameLoadState::FieldExtra)),
            ),
        );
    }
}

fn advance_stopwatch(mut last_state_set: ResMut<StateSetTime>, time: Res<Time>) {
    last_state_set.0.tick(time.delta());
}

#[derive(Message)]
pub struct ChangeBallPos(PointerButton);

impl From<&Pointer<Drag>> for ChangeBallPos {
    fn from(event: &Pointer<Drag>) -> Self {
        Self(event.button)
    }
}

#[derive(Resource, Default)]
struct StateSetTime(Stopwatch);

fn change_ball_pos(
    windows: Query<&Window, With<PrimaryWindow>>,
    socket: Res<Connection>,
    mut game_states: ResMut<GameStates>,
    mut events: MessageReader<ChangeBallPos>,
    camera: Query<(&Camera, &GlobalTransform), With<PrimaryCamera>>,
    mut last_state_set: ResMut<StateSetTime>,
) {
    if !events.read().any(|event| event.0 == PointerButton::Primary) {
        events.clear();
        return;
    }

    events.clear();

    let Some([cam_pos, cursor_dir, plane_normal]) = project_ray_to_plane(camera, windows) else {
        return;
    };

    let target = get_move_object_target(cam_pos, cursor_dir, plane_normal, game_states.current.ball.pos.xzy());
    let ball_vel = (target.xzy() - game_states.current.ball.pos).normalize() * 2000.;
    game_states.current.ball.vel = ball_vel;
    game_states.next.ball.vel = ball_vel;

    last_state_set.0.reset();
    socket.send(SendableUdp::State(game_states.next.clone())).unwrap();
}

#[derive(Message)]
pub struct ChangeCarPos(PointerButton, Entity);

impl From<&Pointer<Drag>> for ChangeCarPos {
    fn from(event: &Pointer<Drag>) -> Self {
        Self(event.button, event.event_target())
    }
}

fn change_car_pos(
    cars: Query<&Car>,
    windows: Query<&Window, With<PrimaryWindow>>,
    socket: Res<Connection>,
    mut game_states: ResMut<GameStates>,
    mut events: MessageReader<ChangeCarPos>,
    camera: Query<(&Camera, &GlobalTransform), With<PrimaryCamera>>,
    mut last_state_set: ResMut<StateSetTime>,
) {
    let Some([cam_pos, cursor_dir, plane_normal]) = project_ray_to_plane(camera, windows) else {
        events.clear();
        return;
    };

    let mut set_state = false;
    for event in events.read() {
        if event.0 != PointerButton::Primary {
            continue;
        }

        let Ok(car_id) = cars.get(event.1).map(Car::id) else {
            return;
        };

        let Some(current_car) = game_states.current.cars.iter_mut().find(|car| car.id == car_id) else {
            return;
        };

        set_state = true;

        let target = get_move_object_target(cam_pos, cursor_dir, plane_normal, current_car.state.pos.xzy());
        let car_vel = (target.xzy() - current_car.state.pos).normalize() * 2000.;
        current_car.state.vel = car_vel;

        if let Some(next_car) = game_states.next.cars.iter_mut().find(|car| car.id == car_id) {
            next_car.state.vel = car_vel;
        };
    }

    if !set_state {
        return;
    }

    last_state_set.0.reset();
    socket.send(SendableUdp::State(game_states.next.clone())).unwrap();
}

#[derive(Message)]
pub struct CarClicked(PointerButton, Entity);

impl From<&Pointer<Click>> for CarClicked {
    fn from(event: &Pointer<Click>) -> Self {
        Self(event.button, event.event_target())
    }
}

fn handle_car_clicked(mut events: MessageReader<CarClicked>, mut enable_car_info: ResMut<EnableCarInfo>, cars: Query<&Car>) {
    for event in events.read() {
        if event.0 != PointerButton::Secondary {
            continue;
        }

        if let Ok(car) = cars.get(event.1) {
            enable_car_info.toggle(car.id());
        }
    }
}

fn project_ray_to_plane(
    camera: Query<(&Camera, &GlobalTransform), With<PrimaryCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) -> Option<[Vec3A; 3]> {
    let (camera, global_transform) = camera.single().unwrap();
    let cursor_coords = windows.single().unwrap().cursor_position()?;

    // Get the ray that goes from the camera through the cursor
    let global_ray = camera.viewport_to_world(global_transform, cursor_coords).ok()?;

    let cam_pos = Vec3A::from(global_ray.origin);
    let cursor_dir = Vec3A::from(Vec3::from(global_ray.direction));

    // define a plane that intersects the ball and is perpendicular to the camera direction
    let plane_normal = (global_transform.affine().matrix3 * Vec3A::Z).normalize();

    Some([cam_pos, cursor_dir, plane_normal])
}

fn get_move_object_target(cam_pos: Vec3A, cursor_dir: Vec3A, plane_normal: Vec3A, plane_point: Vec3A) -> Vec3A {
    // get projection factor
    let lambda = (plane_point - cam_pos).dot(plane_normal) / plane_normal.dot(cursor_dir);

    // project cursor ray onto plane
    cam_pos + lambda * cursor_dir
}

#[derive(Message)]
pub struct BallClicked(PointerButton);

impl From<&Pointer<Click>> for BallClicked {
    fn from(event: &Pointer<Click>) -> Self {
        Self(event.button)
    }
}

fn handle_ball_clicked(mut events: MessageReader<BallClicked>, mut enable_ball_info: ResMut<EnableBallInfo>) {
    // ensure that it was an odd amount of right clicks
    // e.x. right click -> open then right click -> close (an event amount of clicks) wouldn't change the state
    if events.read().filter(|event| event.0 == PointerButton::Secondary).count() % 2 == 0 {
        return;
    }

    enable_ball_info.toggle();
}

#[derive(Message)]
pub struct BoostPadClicked(PointerButton, Entity);

impl From<&Pointer<Click>> for BoostPadClicked {
    fn from(event: &Pointer<Click>) -> Self {
        Self(event.button, event.event_target())
    }
}

fn handle_boost_pad_clicked(
    mut events: MessageReader<BoostPadClicked>,
    mut enable_boost_pad_info: ResMut<EnablePadInfo>,
    boost_pads: Query<&BoostPadI>,
) {
    for event in events.read() {
        if event.0 != PointerButton::Secondary {
            continue;
        }

        if let Ok(boost_pad) = boost_pads.get(event.1) {
            enable_boost_pad_info.toggle(boost_pad.idx());
        }
    }
}

fn load_extra_field(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<NextState<GameLoadState>>,
    ball_assets: Res<BallAssets>,
    assets: Res<AssetServer>,
) {
    // load a glowing ball
    let initial_ball_color = Color::srgb(0.3, 0.3, 0.3);

    let (ball_color, ball_texture) = match assets.get_load_state(&ball_assets.ball_diffuse) {
        Some(LoadState::Failed(_)) => (Color::from(css::DARK_GRAY), None),
        _ => (Color::WHITE, Some(ball_assets.ball_diffuse.clone())),
    };

    let ball_material = StandardMaterial {
        base_color: ball_color,
        base_color_texture: ball_texture,
        // normal_map_texture: Some(ball_assets.ball_normal.clone()),
        // occlusion_texture: Some(ball_assets.ball_occlude.clone()),
        perceptual_roughness: 0.7,
        reflectance: 0.25,
        ..default()
    };

    let ball_mesh = match assets.get_load_state(&ball_assets.ball) {
        Some(LoadState::Failed(_)) => meshes.add(Sphere::new(91.25)),
        _ => ball_assets.ball.clone(),
    };

    commands
        .spawn((
            Ball,
            SimplifiedMesh(meshes.add(Sphere::new(95.))),
            Transform::from_xyz(0., 92., 0.),
            #[cfg(debug_assertions)]
            EntityName::from("ball"),
            Pickable::default(),
            Mesh3d(ball_mesh),
            MeshMaterial3d(materials.add(ball_material)),
            children![PointLight {
                color: initial_ball_color,
                intensity: 200_000_000.,
                range: 1000.,
                ..default()
            }],
        ))
        .observe(target_insert::<Pointer<Over>>(HighlightedEntity))
        .observe(target_remove::<Pointer<Out>, HighlightedEntity>)
        .observe(write_message::<Pointer<Drag>, ChangeBallPos>)
        .observe(write_message::<Pointer<Click>, BallClicked>);

    state.set(GameLoadState::Field);
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
                let [x, y, z] = self.rotation.unwrap_or_default();
                Quat::from_euler(EulerRot::ZYX, z.to_radians(), -y.to_radians(), x.to_radians())
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
#[require(Mesh3d, MeshMaterial3d<StandardMaterial>, NotShadowCaster)]
pub struct StaticFieldEntity;

flate!(pub static STADIUM_P_LAYOUT: str from "stadiums/Stadium_P_MeshObjects.json");
flate!(pub static HOOPS_STADIUM_P_LAYOUT: str from "stadiums/HoopsStadium_P_MeshObjects.json");
flate!(pub static SHATTER_SHOT_P_LAYOUT: str from "stadiums/ShatterShot_P_MeshObjects.json");

fn despawn_old_field(
    mut commands: Commands,
    mut state: ResMut<NextState<GameLoadState>>,
    static_field_entities: Query<Entity, With<StaticFieldEntity>>,
    mut user_pads: ResMut<UserPadStates>,
    mut user_cars: ResMut<UserCarStates>,
    mut user_tiles: ResMut<UserTileStates>,
) {
    user_pads.clear();
    user_cars.clear();
    user_tiles.clear();

    static_field_entities.iter().for_each(|entity| {
        commands.entity(entity).despawn();
    });

    state.set(GameLoadState::Field);
}

#[cfg(feature = "team_goal_barriers")]
fn load_goals(
    game_mode: GameMode,
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    meshes: &mut Assets<Mesh>,
) {
    match game_mode {
        GameMode::Soccar | GameMode::Snowday | GameMode::Heatseeker => {
            commands
                .spawn((
                    Mesh3d(meshes.add(Rectangle::from_size(Vec2::splat(1000.)))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: {
                            let mut color = BLUE_COLOR.with_alpha(0.8);
                            color.blue *= 2.;
                            Color::Srgba(color)
                        },
                        emissive: LinearRgba::from(BLUE_COLOR.with_alpha(0.5)),
                        double_sided: true,
                        cull_mode: None,
                        alpha_mode: AlphaMode::Add,
                        ..default()
                    })),
                    Transform {
                        translation: Vec3::new(0., 321.3875, -5120.),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(0.89 * 2., 0.32 * 2., 0.),
                    },
                    #[cfg(debug_assertions)]
                    EntityName::from("blue_goal"),
                    StaticFieldEntity,
                ))
                .observe(target_insert::<Pointer<Over>>(HighlightedEntity))
                .observe(target_remove::<Pointer<Out>, HighlightedEntity>);

            commands
                .spawn((
                    Mesh3d(meshes.add(Rectangle::from_size(Vec2::splat(1000.)))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: {
                            let mut color = ORANGE_COLOR.with_alpha(0.8);
                            color.red *= 2.;
                            Color::Srgba(color)
                        },
                        emissive: LinearRgba::from(ORANGE_COLOR.with_alpha(0.5)),
                        double_sided: true,
                        cull_mode: None,
                        alpha_mode: AlphaMode::Add,
                        ..default()
                    })),
                    Transform {
                        translation: Vec3::new(0., 321.3875, 5120.),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(0.89 * 2., 0.32 * 2., 0.),
                    },
                    #[cfg(debug_assertions)]
                    EntityName::from("orange_goal"),
                    StaticFieldEntity,
                ))
                .observe(target_insert::<Pointer<Over>>(HighlightedEntity))
                .observe(target_remove::<Pointer<Out>, HighlightedEntity>);
        }
        // TODO: hoops
        _ => {}
    }
}

#[derive(Message)]
pub struct TileClicked(PointerButton, Entity);

impl From<&Pointer<Click>> for TileClicked {
    fn from(event: &Pointer<Click>) -> Self {
        Self(event.button, event.event_target())
    }
}

fn handle_tile_clicked(
    mut events: MessageReader<TileClicked>,
    mut enable_tile_info: ResMut<EnableTileInfo>,
    entities: Query<(&Children, Entity)>,
    tiles: Query<&Tile>,
) {
    for event in events.read() {
        if event.0 != PointerButton::Secondary {
            continue;
        }

        let children = entities.get(event.1).unwrap().0;
        for child in children {
            if let Ok(tile) = tiles.get(*child) {
                let id = (tile.team, tile.index);
                enable_tile_info.toggle(id);
            }
        }
    }
}

fn load_field(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut state: ResMut<NextState<GameLoadState>>,
    mut large_boost_pad_loc_rots: ResMut<LargeBoostPadLocRots>,
    game_mode: Res<GameMode>,
    render_device: Option<Res<RenderDevice>>,
    asset_server: Res<AssetServer>,
    game_states: Res<GameStates>,
) {
    let layout: &str = match *game_mode {
        GameMode::TheVoid => {
            state.set(GameLoadState::None);
            return;
        }
        GameMode::Hoops => &HOOPS_STADIUM_P_LAYOUT,
        GameMode::Dropshot => &SHATTER_SHOT_P_LAYOUT,
        _ => &STADIUM_P_LAYOUT,
    };

    let (the_world, structures) = match *game_mode {
        GameMode::Dropshot => {
            let (the_world,): (Node,) = serde_json::from_str(layout).unwrap();

            (the_world, None)
        }
        _ => {
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
            }

            (the_world, Some(structures))
        }
    };

    #[cfg(debug_assertions)]
    debug_assert_eq!(the_world.name.as_ref(), "TheWorld");

    let persistent_level = &the_world.sub_nodes[0];
    #[cfg(debug_assertions)]
    debug_assert_eq!(persistent_level.name.as_ref(), "PersistentLevel");

    let all_nodes = persistent_level.sub_nodes.iter().chain(
        structures
            .as_ref()
            .map(|s| s.sub_nodes[0].sub_nodes.iter())
            .unwrap_or_default(),
    );

    for obj in all_nodes {
        if let Some(node) = obj.get_info_node() {
            process_info_node(
                &node,
                &asset_server,
                &mut meshes,
                &mut materials,
                &mut large_boost_pad_loc_rots,
                &mut commands,
                &mut images,
                render_device.as_deref(),
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
                &mut images,
                render_device.as_deref(),
            );
        }
    }

    if *game_mode == GameMode::Dropshot {
        let mut middle_mesh = Mesh::new(mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::all());
        middle_mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                Vec3::new(4980.0, 0.0, 2.54736 * 50.0),
                Vec3::new(-4980.0, 0.0, 2.54736 * 50.0),
                Vec3::new(4980.0, 0.0, -2.54736 * 50.0),
                Vec3::new(-4980.0, 0.0, -2.54736 * 50.0),
            ],
        );
        middle_mesh.insert_indices(mesh::Indices::U16(vec![0, 2, 1, 1, 2, 3]));
        middle_mesh.compute_normals();

        commands.spawn((
            Mesh3d(meshes.add(middle_mesh)),
            MeshMaterial3d(materials.add(StandardMaterial::from(Color::from(css::BEIGE)))),
            #[cfg(debug_assertions)]
            EntityName::from("dropshot neutral ground"),
            StaticFieldEntity,
        ));

        let verts = vec![
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -8.85) * 50.,
            Vec3::new(7.6643, 0.0, -4.425) * 50.,
            Vec3::new(7.6643, 0.0, 4.425) * 50.,
            Vec3::new(0.0, 0.0, 8.85) * 50.,
            Vec3::new(-7.6643, 0.0, 4.425) * 50.,
            Vec3::new(-7.6643, 0.0, -4.425) * 50.,
        ];

        let indices = mesh::Indices::U16(vec![0, 2, 1, 0, 3, 2, 0, 4, 3, 0, 5, 4, 0, 6, 5, 0, 1, 6]);

        let mut raw_blue_tile_mesh = Mesh::new(mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::all());
        raw_blue_tile_mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            verts
                .iter()
                .map(|vert| (*vert).with_z((vert.z - 127.0).min(-2.54736 * 50.0) + 127.0))
                .collect::<Vec<_>>(),
        );
        raw_blue_tile_mesh.insert_indices(indices.clone());
        raw_blue_tile_mesh.compute_normals();
        let blue_tile_mesh = meshes.add(raw_blue_tile_mesh);

        let mut raw_orange_tile_mesh = Mesh::new(mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::all());
        raw_orange_tile_mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            verts
                .iter()
                .map(|vert| (*vert).with_z((vert.z + 127.0).max(2.54736 * 50.0) - 127.0))
                .collect::<Vec<_>>(),
        );
        raw_orange_tile_mesh.insert_indices(indices.clone());
        raw_orange_tile_mesh.compute_normals();
        let orange_tile_mesh = meshes.add(raw_orange_tile_mesh);

        let mut raw_full_tile_mesh = Mesh::new(mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::all());
        raw_full_tile_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts);
        raw_full_tile_mesh.insert_indices(indices);
        raw_full_tile_mesh.compute_normals();
        let full_tile_mesh = meshes.add(raw_full_tile_mesh);

        for (i, team_tiles) in game_states.current.tiles.iter().enumerate() {
            let team_color = materials.add(StandardMaterial::from(Color::from(if i == 0 {
                css::BLUE
            } else {
                css::ORANGE
            })));

            for (j, tile) in team_tiles.iter().enumerate() {
                commands
                    .spawn((
                        Mesh3d(if tile.pos.y.abs() < 150.0 {
                            if tile.pos.y.signum().is_sign_positive() {
                                orange_tile_mesh.clone()
                            } else {
                                blue_tile_mesh.clone()
                            }
                        } else {
                            full_tile_mesh.clone()
                        }),
                        MeshMaterial3d(team_color.clone()),
                        Transform::from_translation(tile.pos.to_bevy()),
                        NotShadowCaster,
                        StaticFieldEntity,
                        #[cfg(debug_assertions)]
                        EntityName::from(format!("dropshot_tile_{}", i * 70 + j)),
                        Pickable::default(),
                        children![(
                            Tile { team: i, index: j },
                            Mesh3d(if tile.pos.y.abs() < 150.0 {
                                if tile.pos.y.signum().is_sign_positive() {
                                    orange_tile_mesh.clone()
                                } else {
                                    blue_tile_mesh.clone()
                                }
                            } else {
                                full_tile_mesh.clone()
                            }),
                            MeshMaterial3d(materials.add(StandardMaterial::from(get_tile_color(tile.state)))),
                            NotShadowCaster,
                            Transform::from_translation(Vec3::Y).with_scale(Vec3::splat(0.9)),
                        )],
                    ))
                    .observe(target_insert::<Pointer<Over>>(HighlightedEntity))
                    .observe(target_remove::<Pointer<Out>, HighlightedEntity>)
                    .observe(write_message::<Pointer<Click>, TileClicked>);
            }
        }
    }

    #[cfg(feature = "team_goal_barriers")]
    load_goals(*game_mode, &mut commands, &mut materials, &mut meshes);

    state.set(GameLoadState::None);
}

fn process_info_node(
    node: &InfoNode,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    large_boost_pad_loc_rots: &mut LargeBoostPadLocRots,
    commands: &mut Commands,
    images: &mut Assets<Image>,
    render_device: Option<&RenderDevice>,
) {
    if node.static_mesh.trim().is_empty() {
        return;
    }

    let Some(mesh) = get_mesh_info(&node.static_mesh, meshes) else {
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
        if !node.static_mesh.contains("BreakOut") && BLACKLIST_MESH_MATS.contains(&mat.as_ref()) {
            continue;
        }

        let side_signum = if node.static_mesh.contains("BBall_HoopBackBoard_02") {
            if node.rotation.is_some() { -1 } else { 1 }
        } else {
            node.translation.map(|t| (t[1] as i16).signum()).unwrap_or_default()
        };

        let side = match side_signum.cmp(&0) {
            Ordering::Greater => Some(Team::Orange),
            Ordering::Less => Some(Team::Blue),
            Ordering::Equal => None,
        };

        let mat_name = if !cfg!(feature = "full_load") && node.static_mesh.ends_with("OOBFloor") {
            "OOBFloor_MAT_CUSTOM"
        } else {
            mat.as_ref()
        };

        let material = get_material(mat_name, materials, asset_server, None, side, images, render_device);

        let mut transform = node.get_transform();

        if node.static_mesh.contains("Grass.Grass") || node.static_mesh.contains("Grass_1x1") {
            transform.translation.y += 10.;
        } else if node.static_mesh.contains("BoostPad_Large") {
            large_boost_pad_loc_rots
                .locs
                .push(node.translation.map(ToBevyVecFlat::to_bevy_flat).unwrap_or_default());
            large_boost_pad_loc_rots
                .rots
                .push(node.rotation.map(|r| -r[1]).unwrap_or_default());
        }

        let mut obj = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            transform,
            #[cfg(debug_assertions)]
            EntityName::from(format!("{} | {mat}", node.static_mesh)),
            StaticFieldEntity,
            #[cfg(debug_assertions)]
            Pickable::default(),
        ));
        #[cfg(debug_assertions)]
        obj.observe(target_insert::<Pointer<Over>>(HighlightedEntity))
            .observe(target_remove::<Pointer<Out>, HighlightedEntity>);

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
#[derive(Clone, Debug, Default, bincode::Encode, bincode::Decode)]
pub struct MeshBuilder {
    ids: Vec<u32>,
    verts: Vec<f32>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    num_materials: usize,
    mat_ids: Vec<usize>,
}

impl MeshBuilder {
    pub fn create_cache(&self, path: &Path) {
        create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = File::create(path).unwrap();
        bincode::encode_into_std_write(self, &mut file, bincode::config::legacy()).unwrap();
    }

    pub fn from_cache<R: Read>(mut reader: R) -> Self {
        bincode::decode_from_std_read(&mut reader, bincode::config::legacy()).unwrap()
    }

    #[must_use]
    // Build the Bevy Mesh
    pub fn build_meshes(self) -> Vec<Mesh> {
        if self.num_materials < 2 {
            return vec![self.build_mesh()];
        }

        let verts = self
            .verts
            .chunks_exact(3)
            .map(|chunk| [chunk[0], chunk[1], chunk[2]])
            .collect::<Vec<_>>();

        (0..self.num_materials)
            .map(|mat_id| {
                let mut mesh = Mesh::new(mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::default());

                let ids: Vec<_> = self
                    .ids
                    .chunks_exact(3)
                    .filter(|ids| ids.iter().copied().any(|id| self.mat_ids[id as usize] == mat_id))
                    .flatten()
                    .copied()
                    .collect();

                if self.colors.len() == verts.len() {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors.clone());
                }

                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts.clone());
                mesh.insert_indices(mesh::Indices::U32(ids));
                mesh.compute_smooth_normals();

                if !self.uvs.is_empty() {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs.clone());
                    mesh.generate_tangents().unwrap();
                }

                mesh
            })
            .collect()
    }

    #[must_use]
    // Build the Bevy Mesh
    pub fn build_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::default());

        let verts = self
            .verts
            .chunks_exact(3)
            .map(|chunk| [chunk[0], chunk[1], chunk[2]])
            .collect::<Vec<_>>();

        if self.colors.len() == verts.len() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts);
        mesh.insert_indices(mesh::Indices::U32(self.ids));
        mesh.compute_smooth_normals();

        if !self.uvs.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
            mesh.generate_tangents().unwrap();
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
                    read_vertices(&chunk_data, chunk_data_count, &mut verts, &mut uvs, &mut mat_ids);
                    debug_assert_eq!(verts.len() / 3, chunk_data_count);
                    debug_assert_eq!(verts.len() % 3, 0);
                }
                "VTXW0000" => {
                    read_wedges(&chunk_data, chunk_data_count, &mut wedges, &mut uvs, &mut mat_ids);
                    debug_assert_eq!(wedges.len(), chunk_data_count);
                }
                "FACE0000" => {
                    read_faces(&chunk_data, chunk_data_count, &wedges, &mut ids);
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
    ids: &[u32],
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
