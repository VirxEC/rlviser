use crate::{
    assets::{get_material, get_mesh_info, BoostPickupGlows, CarWheelMesh},
    bytes::{FromBytes, ToBytes, ToBytesExact},
    camera::{BoostAmount, HighlightedEntity, PrimaryCamera, TimeDisplay, BOOST_INDICATOR_FONT_SIZE, BOOST_INDICATOR_POS},
    mesh::{BoostPadClicked, CarClicked, ChangeCarPos, LargeBoostPadLocRots},
    morton::Morton,
    renderer::{RenderGroups, RenderMessage, UdpRendererPlugin},
    rocketsim::{CarInfo, GameMode, GameState, Team},
    settings::{
        options::{BallCam, CalcBallRot, Extrapolation, GameSpeed, ShowTime, UiOverlayScale},
        state_setting::UserCarStates,
    },
    GameLoadState, ServerPort,
};
use ahash::HashMap;
use bevy::{
    app::AppExit,
    asset::LoadState,
    math::{Mat3A, Vec3A},
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
    window::PrimaryWindow,
};
use bevy_mod_picking::{backends::raycast::RaycastPickable, prelude::*};
use bevy_vector_shapes::prelude::*;
use std::{
    cmp::Ordering,
    f32::consts::PI,
    fs,
    net::{IpAddr, SocketAddr, UdpSocket},
    str::FromStr,
    time::Duration,
};

#[cfg(debug_assertions)]
use crate::camera::EntityName;

#[derive(Component)]
pub struct BoostPadI(u64);

impl BoostPadI {
    #[inline]
    pub const fn id(&self) -> u64 {
        self.0
    }
}

#[derive(Component)]
pub struct Ball;

#[derive(Component)]
pub struct Car(u32);

impl Car {
    #[inline]
    pub const fn id(&self) -> u32 {
        self.0
    }
}

#[derive(Resource)]
struct DirectorTimer(Timer);

#[derive(Resource)]
pub struct Connection(pub UdpSocket, pub SocketAddr);

fn establish_connection(port: Res<ServerPort>, mut commands: Commands, mut state: ResMut<NextState<GameLoadState>>) {
    let socket_addr = SocketAddr::new(IpAddr::from_str("127.0.0.1").unwrap(), port.primary_port);
    let socket = UdpSocket::bind(("0.0.0.0", port.secondary_port)).unwrap();
    socket.send_to(&[UdpPacketTypes::Connection as u8], socket_addr).unwrap();
    socket.set_nonblocking(true).unwrap();
    commands.insert_resource(Connection(socket, socket_addr));
    state.set(GameLoadState::FieldExtra);
}

pub trait ToBevyVec {
    fn to_bevy(self) -> Vec3;
}

pub trait ToBevyVecFlat {
    fn to_bevy_flat(self) -> Vec2;
}

impl ToBevyVecFlat for [f32; 3] {
    #[inline]
    fn to_bevy_flat(self) -> Vec2 {
        Vec2::new(self[0], self[1])
    }
}

impl ToBevyVec for [f32; 3] {
    #[inline]
    fn to_bevy(self) -> Vec3 {
        Vec3::new(self[0], self[2], self[1])
    }
}

impl ToBevyVec for Vec3A {
    #[inline]
    fn to_bevy(self) -> Vec3 {
        Vec3::new(self.x, self.z, self.y)
    }
}

impl ToBevyVec for Vec3 {
    #[inline]
    fn to_bevy(self) -> Vec3 {
        Vec3::new(self.x, self.z, self.y)
    }
}

trait ToBevyMat {
    fn to_bevy(self) -> Quat;
}

impl ToBevyMat for Mat3A {
    #[inline]
    fn to_bevy(self) -> Quat {
        let quat = Quat::from_mat3a(&self);
        Quat::from_xyzw(quat.x, quat.z, quat.y, -quat.w)
    }
}

const NUM_CAR_BODIES: usize = 6;

const CAR_BODIES: [&str; NUM_CAR_BODIES] = [
    "Body_Octane.SkeletalMesh3.Body_Octane_SK",
    "Body_MuscleCar.SkeletalMesh3.Body_MuscleCar_SK",
    "Body_Darkcar.SkeletalMesh3.Body_Darkcar_SK",
    "Body_Force.SkeletalMesh3.Body_Force_PremiumSkin_SK",
    "Body_Venom.SkeletalMesh3.Body_Venom_PremiumSkin_SK",
    "Body_Vanquish.SkeletalMesh3.Body_Merc_PremiumSkin_SK",
];

#[cfg(debug_assertions)]
const CAR_BODY_NAMES: [&str; NUM_CAR_BODIES] = [
    "octane_body",
    "dominus_body",
    "plank_body",
    "breakout_body",
    "hybrid_body",
    "merc_body",
];

pub const BLUE_COLOR: Color = if cfg!(feature = "full_load") {
    Color::rgb(0.03, 0.09, 0.79)
} else {
    Color::rgb(0.01, 0.03, 0.39)
};

pub const ORANGE_COLOR: Color = if cfg!(feature = "full_load") {
    Color::rgb(0.41, 0.21, 0.01)
} else {
    Color::rgb(0.82, 0.42, 0.02)
};

#[inline]
/// Use colors that are a bit darker if we don't have the `full_load` feature
const fn get_color_from_team(team: Team) -> Color {
    match team {
        Team::Blue => BLUE_COLOR,
        Team::Orange => ORANGE_COLOR,
    }
}

#[derive(Component)]
pub struct CarBoost;

#[derive(Component)]
struct CarWheel {
    front: bool,
    left: bool,
}

impl CarWheel {
    fn new(front: bool, left: bool) -> Self {
        Self { front, left }
    }
}

fn spawn_car(
    car_info: &CarInfo,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    car_wheel_mesh: &CarWheelMesh,
) {
    let hitbox = car_info.config.hitbox_size.to_bevy();
    let base_color = get_color_from_team(car_info.team);

    let car_index = if (120f32..121.).contains(&hitbox.x) {
        // octane
        0
    } else if (130f32..131.).contains(&hitbox.x) {
        // dominus
        1
    } else if (131f32..132.).contains(&hitbox.x) {
        // plank
        2
    } else if (133f32..134.).contains(&hitbox.x) {
        // breakout
        3
    } else if (129f32..130.).contains(&hitbox.x) {
        // hybrid
        4
    } else if (123f32..124.).contains(&hitbox.x) {
        // merc
        5
    } else {
        // spawn octane by default
        0
    };

    #[cfg(debug_assertions)]
    let name = CAR_BODY_NAMES[car_index];
    let mesh_id = CAR_BODIES[car_index];

    let mesh_info = get_mesh_info(mesh_id, meshes)
        .unwrap_or_else(|| vec![meshes.add(Cuboid::new(hitbox.x * 2., hitbox.y * 2., hitbox.z * 2.))]);

    commands
        .spawn((
            Car(car_info.id),
            PbrBundle {
                mesh: meshes.add(Cuboid::new(hitbox.x * 2., hitbox.y * 3., hitbox.z * 2.)),
                material: materials.add(StandardMaterial {
                    base_color: Color::NONE,
                    alpha_mode: AlphaMode::Add,
                    unlit: true,
                    ..default()
                }),
                ..default()
            },
            #[cfg(debug_assertions)]
            EntityName::from(name),
            RaycastPickable,
            On::<Pointer<Over>>::target_insert(HighlightedEntity),
            On::<Pointer<Out>>::target_remove::<HighlightedEntity>(),
            On::<Pointer<Drag>>::send_event::<ChangeCarPos>(),
            On::<Pointer<Click>>::send_event::<CarClicked>(),
        ))
        .with_children(|parent| {
            const CAR_BOOST_LENGTH: f32 = 50.;

            if cfg!(feature = "full_load") {
                let mesh_materials = get_car_mesh_materials(mesh_id, materials, asset_server, base_color);

                mesh_info
                    .into_iter()
                    .zip(mesh_materials)
                    .map(|(mesh, material)| PbrBundle {
                        mesh,
                        material,
                        ..default()
                    })
                    .for_each(|bundle| {
                        parent.spawn(bundle);
                    });
            } else {
                let material = materials.add(base_color);

                mesh_info
                    .into_iter()
                    .map(|mesh| PbrBundle {
                        mesh,
                        material: material.clone(),
                        ..default()
                    })
                    .for_each(|bundle| {
                        parent.spawn(bundle);
                    });
            }

            parent.spawn((
                PbrBundle {
                    mesh: meshes.add(Cylinder::new(10., CAR_BOOST_LENGTH)),
                    material: materials.add(StandardMaterial {
                        base_color: Color::rgba(1., 1., 0., 0.),
                        alpha_mode: AlphaMode::Add,
                        cull_mode: None,
                        ..default()
                    }),
                    transform: Transform {
                        translation: Vec3::new((hitbox.x + CAR_BOOST_LENGTH) / -2., hitbox.y / 2., 0.),
                        rotation: Quat::from_rotation_z(PI / 2.),
                        ..default()
                    },
                    ..default()
                },
                CarBoost,
            ));

            let wheel_material = materials.add(base_color);
            let wheel_pairs = [car_info.config.front_wheels, car_info.config.back_wheels];

            for (i, wheel_pair) in wheel_pairs.iter().enumerate() {
                let wheel_offset = -Vec3::Y * (wheel_pair.suspension_rest_length - 12.);

                for side in 0..=1 {
                    let offset = Vec3::new(1., 1., 1. - (2. * side as f32));

                    parent.spawn((
                        PbrBundle {
                            mesh: car_wheel_mesh.mesh.clone(),
                            material: wheel_material.clone(),
                            transform: Transform {
                                translation: wheel_pair.connection_point_offset.to_bevy() * offset + wheel_offset,
                                rotation: Quat::from_rotation_x(PI * side as f32),
                                ..default()
                            },
                            ..default()
                        },
                        CarWheel::new(i == 0, side == 0),
                    ));
                }
            }
        });
}

fn get_car_mesh_materials(
    mesh_id: &str,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    base_color: Color,
) -> Vec<Handle<StandardMaterial>> {
    let mesh_path = mesh_id.replace('.', "/");
    let props = fs::read_to_string(format!("./assets/{mesh_path}.props.txt")).unwrap();
    let mut mesh_materials = Vec::with_capacity(2);

    let mut inside_mats = false;
    for line in props.lines() {
        if !inside_mats {
            if line.starts_with("Materials[") {
                inside_mats = true;
            }
            continue;
        }

        if line.starts_with('{') {
            continue;
        }

        if line.starts_with('}') {
            break;
        }

        let material_name = line.split('\'').nth(1).unwrap();

        mesh_materials.push(get_material(material_name, materials, asset_server, Some(base_color)));
    }
    mesh_materials
}

#[repr(u8)]
pub enum UdpPacketTypes {
    Quit,
    GameState,
    Connection,
    Paused,
    Speed,
    Render,
}

impl UdpPacketTypes {
    const fn new(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Quit),
            1 => Some(Self::GameState),
            2 => Some(Self::Connection),
            3 => Some(Self::Paused),
            4 => Some(Self::Speed),
            5 => Some(Self::Render),
            _ => None,
        }
    }
}

#[derive(Event)]
pub struct SpeedUpdate(pub f32);

#[derive(Event)]
pub struct PausedUpdate(pub bool);

fn handle_udp(
    socket: Res<Connection>,
    game_speed: Res<GameSpeed>,
    calc_ball_rot: Res<CalcBallRot>,
    mut game_state: ResMut<GameState>,
    mut exit: EventWriter<AppExit>,
    mut packet_updated: ResMut<PacketUpdated>,
    mut render_groups: ResMut<RenderGroups>,
    mut speed_update: EventWriter<SpeedUpdate>,
    mut paused_update: EventWriter<PausedUpdate>,
) {
    const BYTE_BUFFER: [u8; 1] = [0];

    let mut packet_type = BYTE_BUFFER;

    packet_updated.0 = false;
    let mut buf = Vec::new();
    let mut render_buf = Vec::new();

    while socket.0.recv_from(&mut packet_type).is_ok() {
        let Some(packet_type) = UdpPacketTypes::new(packet_type[0]) else {
            return;
        };

        match packet_type {
            UdpPacketTypes::Quit => {
                exit.send(AppExit);
                return;
            }
            UdpPacketTypes::GameState => {
                const INITIAL_BUFFER: [u8; GameState::MIN_NUM_BYTES] = [0; GameState::MIN_NUM_BYTES];
                let mut initial_buffer = INITIAL_BUFFER;

                // wait until we receive the packet
                // it should arrive VERY quickly, so a loop with no delay is fine
                // if it doesn't, then there are other problems lol
                // UPDATE: Windows throws a specific error that we need to look for
                // despite the fact that it actually worked

                #[cfg(windows)]
                {
                    while let Err(e) = socket.0.peek_from(&mut initial_buffer) {
                        if let Some(code) = e.raw_os_error() {
                            if code == 10040 {
                                break;
                            }
                        }
                    }
                }

                #[cfg(not(windows))]
                {
                    while socket.0.peek_from(&mut initial_buffer).is_err() {}
                }

                let new_tick_count = GameState::read_tick_count(&initial_buffer);
                if new_tick_count > 1 && game_state.tick_count > new_tick_count {
                    drop(socket.0.recv_from(&mut [0]));
                    return;
                }

                buf.resize(GameState::get_num_bytes(&initial_buffer), 0);
                if socket.0.recv_from(&mut buf).is_err() {
                    return;
                }
            }
            UdpPacketTypes::Render => {
                const INITIAL_BUFFER: [u8; RenderMessage::MIN_NUM_BYTES] = [0; RenderMessage::MIN_NUM_BYTES];
                let mut initial_buffer = INITIAL_BUFFER;

                #[cfg(windows)]
                {
                    while let Err(e) = socket.0.peek_from(&mut initial_buffer) {
                        if let Some(code) = e.raw_os_error() {
                            if code == 10040 {
                                break;
                            }
                        }
                    }
                }

                #[cfg(not(windows))]
                {
                    while socket.0.peek_from(&mut initial_buffer).is_err() {}
                }

                render_buf.resize(RenderMessage::get_num_bytes(&initial_buffer), 0);
                if socket.0.recv_from(&mut render_buf).is_err() {
                    return;
                }

                let render_message = RenderMessage::from_bytes(&render_buf);

                match render_message {
                    RenderMessage::AddRender(group_id, renders) => {
                        render_groups.groups.insert(group_id, renders);
                    }
                    RenderMessage::RemoveRender(group_id) => {
                        render_groups.groups.remove(&group_id);
                    }
                }
            }
            UdpPacketTypes::Speed => {
                const SPEED_BUFFER: [u8; 4] = [0; 4];
                let mut speed_buffer = SPEED_BUFFER;
                if socket.0.recv_from(&mut speed_buffer).is_err() {
                    return;
                }

                let speed = f32::from_le_bytes(speed_buffer);
                speed_update.send(SpeedUpdate(speed));
            }
            UdpPacketTypes::Paused => {
                let mut paused_buffer = BYTE_BUFFER;
                if socket.0.recv_from(&mut paused_buffer).is_err() {
                    return;
                }

                let paused = paused_buffer[0] != 0;
                paused_update.send(PausedUpdate(paused));
            }
            UdpPacketTypes::Connection => {
                let paused = [game_speed.paused as u8];
                socket.0.send_to(&[UdpPacketTypes::Paused as u8], socket.1).unwrap();
                socket.0.send_to(&paused, socket.1).unwrap();

                let speed = game_speed.speed.to_bytes();
                socket.0.send_to(&[UdpPacketTypes::Speed as u8], socket.1).unwrap();
                socket.0.send_to(&speed, socket.1).unwrap();
            }
        }
    }

    if buf.is_empty() {
        return;
    }

    packet_updated.0 = true;

    let mut new_game_state = GameState::from_bytes(&buf);

    if calc_ball_rot.0 {
        new_game_state.ball.rot_mat = game_state.ball.rot_mat;
    };

    *game_state = new_game_state;
}

fn update_ball(
    state: Res<GameState>,
    mut ball: Query<(&mut Transform, &Children), With<Ball>>,
    mut point_light: Query<&mut PointLight>,
) {
    let Ok((mut transform, children)) = ball.get_single_mut() else {
        return;
    };

    let new_pos = state.ball.pos.to_bevy();
    transform.translation = new_pos;

    let mut point_light = point_light.get_mut(children.first().copied().unwrap()).unwrap();

    let amount = (transform.translation.z.abs() + 500.) / 3500.;
    point_light.color = if new_pos.z > 0. {
        Color::rgb(amount.max(0.5), (amount * (2. / 3.)).max(0.5), 0.5)
    } else {
        Color::rgb(0.5, 0.5, amount.max(0.5))
    };

    transform.rotation = state.ball.rot_mat.to_bevy();
}

const MIN_DIST_FROM_BALL: f32 = 200.;
const MIN_DIST_FROM_BALL_SQ: f32 = MIN_DIST_FROM_BALL * MIN_DIST_FROM_BALL;

const MIN_CAMERA_BALLCAM_HEIGHT: f32 = 20.;

fn update_car(state: Res<GameState>, mut cars: Query<(&mut Transform, &Car)>) {
    for (mut car_transform, car) in &mut cars {
        let Some(target_car) = state.cars.iter().find(|car_info| car.0 == car_info.id) else {
            continue;
        };

        car_transform.translation = target_car.state.pos.to_bevy();
        car_transform.rotation = target_car.state.rot_mat.to_bevy();
    }
}

fn update_car_extra(
    state: Res<GameState>,
    car_entities: Query<(Entity, &Car)>,
    mut cars: Query<(&Car, &Children)>,
    mut car_boosts: Query<&Handle<StandardMaterial>, With<CarBoost>>,
    mut car_materials: Query<&Handle<StandardMaterial>, (With<Car>, Without<CarBoost>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last_boost_states: Local<Vec<u32>>,
    mut last_demoed_states: Local<Vec<u32>>,
    mut last_boost_amounts: Local<HashMap<u32, f32>>,
) {
    for (car, children) in &mut cars {
        let Some(target_car) = state.cars.iter().find(|car_info| car.0 == car_info.id) else {
            continue;
        };

        let last_demoed = last_demoed_states.iter().any(|&id| id == car.id());

        if target_car.state.is_demoed != last_demoed {
            for (entity, car) in &car_entities {
                if car.0 == target_car.id {
                    let material_handle = car_materials.get_mut(entity).unwrap();
                    let material = materials.get_mut(material_handle).unwrap();
                    if target_car.state.is_demoed {
                        material.base_color.set_a(0.);
                        last_demoed_states.push(car.id());
                    } else {
                        material.base_color.set_a(1.);
                        last_demoed_states.retain(|&id| id != car.id());
                    }
                }
            }
        }

        let last_boost_amount = last_boost_amounts
            .insert(car.id(), target_car.state.boost)
            .unwrap_or_default();

        let is_boosting = !target_car.state.is_demoed
            && target_car.state.boost > f32::EPSILON
            && (target_car.state.last_controls.boost || last_boost_amount > target_car.state.boost);
        let last_boosted = last_boost_states.iter().any(|&id| id == car.id());

        if is_boosting != last_boosted {
            for child in children {
                let Ok(material_handle) = car_boosts.get_mut(*child) else {
                    continue;
                };

                let material = materials.get_mut(material_handle).unwrap();
                if is_boosting {
                    material.base_color.set_a(0.7);
                    last_boost_states.push(car.id());
                } else {
                    material.base_color.set_a(0.0);
                    last_boost_states.retain(|&id| id != car.id());
                }
            }
        }
    }
}

fn update_car_wheels(
    state: Res<GameState>,
    cars: Query<(&Transform, &Car, &Children)>,
    car_wheels: Query<(&mut Transform, &CarWheel), Without<Car>>,
    game_speed: Res<GameSpeed>,
    time: Res<Time>,
    key: Res<ButtonInput<KeyCode>>,
) {
    if game_speed.paused {
        return;
    }

    let delta_time = if key.pressed(KeyCode::KeyI) {
        game_speed.speed / state.tick_rate
    } else {
        time.delta_seconds() * game_speed.speed
    };

    calc_car_wheel_update(&state, cars, car_wheels, delta_time);
}

fn calc_car_wheel_update(
    state: &GameState,
    mut cars: Query<(&Transform, &Car, &Children)>,
    mut car_wheels: Query<(&mut Transform, &CarWheel), Without<Car>>,
    delta_time: f32,
) {
    for (car_transform, car, children) in &mut cars {
        let Some(target_car) = state.cars.iter().find(|car_info| car.0 == car_info.id) else {
            continue;
        };

        for child in children {
            let Ok((mut wheel_transform, data)) = car_wheels.get_mut(*child) else {
                continue;
            };

            let wheel_radius = if data.front {
                target_car.config.front_wheels.wheel_radius
            } else {
                target_car.config.back_wheels.wheel_radius
            };

            let car_vel = target_car.state.vel.to_bevy();
            let mut angular_velocity = car_vel.length() * delta_time / wheel_radius;

            if data.left {
                angular_velocity *= -1.;
            }

            if target_car.state.is_on_ground || target_car.state.wheels_with_contact.into_iter().any(|b| b) {
                // determine if the velocity is in the same direction as the car's forward vector
                let forward = car_transform.rotation.mul_vec3(Vec3::X);
                let forward_dot = forward.dot(car_vel);
                let forward_dir = forward_dot.signum();

                angular_velocity *= forward_dir;
            } else {
                angular_velocity *= target_car.state.last_controls.throttle;
            }

            wheel_transform.rotation *= Quat::from_rotation_z(angular_velocity);
        }
    }
}

fn pre_update_car(
    cars: Query<&Car>,
    state: Res<GameState>,
    asset_server: Res<AssetServer>,
    car_entities: Query<(Entity, &Car)>,
    commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut user_cars: ResMut<UserCarStates>,
    car_wheel_mesh: Res<CarWheelMesh>,
) {
    correct_car_count(
        &cars,
        &state,
        &car_entities,
        &mut user_cars,
        commands,
        &mut meshes,
        &mut materials,
        &asset_server,
        &car_wheel_mesh,
    );
}

fn post_update_car(
    time: Res<Time>,
    state: Res<GameState>,
    ballcam: Res<BallCam>,
    mut cars: Query<(&mut Transform, &Car)>,
    mut camera_query: Query<(&mut PrimaryCamera, &mut Transform), Without<Car>>,
    mut timer: ResMut<DirectorTimer>,
) {
    timer.0.tick(time.delta());

    let (mut primary_camera, mut camera_transform) = camera_query.single_mut();

    let car_id = match primary_camera.as_mut() {
        PrimaryCamera::TrackCar(id) => *id,
        PrimaryCamera::Director(id) => {
            if *id == 0 || timer.0.finished() {
                // get the car closest to the ball
                let mut min_dist = f32::MAX;
                let mut new_id = *id;
                for car in &*state.cars {
                    let dist = car.state.pos.distance_squared(state.ball.pos);
                    if dist < min_dist {
                        new_id = car.id;
                        min_dist = dist;
                    }
                }

                *id = new_id;
            }

            *id
        }
        PrimaryCamera::Spectator => 0,
    };

    for (car_transform, car) in &mut cars {
        let Some(target_car) = state.cars.iter().find(|car_info| car.0 == car_info.id) else {
            continue;
        };

        if car_id == car.id() {
            let camera_transform = camera_transform.as_mut();

            if ballcam.enabled
                && (!target_car.state.is_on_ground
                    || target_car.state.pos.distance_squared(state.ball.pos) > MIN_DIST_FROM_BALL_SQ)
            {
                let ball_pos = state.ball.pos.to_bevy();
                camera_transform.translation =
                    car_transform.translation + (car_transform.translation - ball_pos).normalize() * 300.;
                camera_transform.look_at(ball_pos, Vec3::Y);
                camera_transform.translation += camera_transform.up() * 150.;
                camera_transform.look_at(ball_pos, Vec3::Y);

                if camera_transform.translation.y < MIN_CAMERA_BALLCAM_HEIGHT {
                    camera_transform.translation.y = MIN_CAMERA_BALLCAM_HEIGHT;
                }
            } else {
                let car_look = Vec3::new(target_car.state.vel.x, 0., target_car.state.vel.y)
                    .try_normalize()
                    .unwrap_or_else(|| car_transform.forward().into());
                camera_transform.translation = car_transform.translation - car_look * 280. + Vec3::Y * 110.;
                camera_transform.look_to(car_look, Vec3::Y);
                camera_transform.rotation *= Quat::from_rotation_x(-PI / 30.);
            }
        }
    }
}

fn correct_car_count(
    cars: &Query<&Car>,
    state: &GameState,
    car_entities: &Query<(Entity, &Car)>,
    user_cars: &mut UserCarStates,
    mut commands: Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    car_wheel_mesh: &CarWheelMesh,
) {
    match cars.iter().count().cmp(&state.cars.len()) {
        Ordering::Greater => {
            for (entity, car) in car_entities {
                if !state.cars.iter().any(|car_info| car.0 == car_info.id) {
                    user_cars.remove(car.0);
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
        Ordering::Less => {
            let non_existant_cars = state
                .cars
                .iter()
                .filter(|car_info| !cars.iter().any(|id| id.0 == car_info.id));

            for car_info in non_existant_cars {
                spawn_car(car_info, &mut commands, meshes, materials, asset_server, car_wheel_mesh);
            }
        }
        Ordering::Equal => {}
    }
}

fn update_pads_count(
    state: Res<GameState>,
    asset_server: Res<AssetServer>,
    pads: Query<(Entity, &BoostPadI)>,
    pad_glows: Res<BoostPickupGlows>,
    large_boost_pad_loc_rots: Res<LargeBoostPadLocRots>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    if pads.iter().count() != state.pads.len() && !large_boost_pad_loc_rots.rots.is_empty() {
        let morton_generator = Morton::default();

        // The number of pads shouldn't change often
        // There's also not an easy way to determine
        // if a previous pad a new pad are same pad
        // It is the easiest to despawn and respawn all pads
        for (entity, _) in pads.iter() {
            commands.entity(entity).despawn_recursive();
        }
        let hitbox_material = materials.add(Color::NONE);

        let large_pad_mesh = match asset_server.get_load_state(&pad_glows.large) {
            Some(LoadState::Failed) | None => pad_glows.large_hitbox.clone(),
            _ => pad_glows.large.clone(),
        };

        let small_pad_mesh = match asset_server.get_load_state(&pad_glows.small) {
            Some(LoadState::Failed) | None => pad_glows.small_hitbox.clone(),
            _ => pad_glows.small.clone(),
        };

        for pad in state.pads.iter() {
            let code = morton_generator.get_code(pad.position);
            let mut transform = Transform::from_translation(pad.position.to_bevy() - Vec3::Y * 70.);

            let (visual_mesh, hitbox) = if pad.is_big {
                let rotation = large_boost_pad_loc_rots
                    .locs
                    .iter()
                    .enumerate()
                    .find(|(_, loc)| loc.distance_squared(pad.position.xy()) < 25.)
                    .map(|(i, _)| large_boost_pad_loc_rots.rots[i]);
                transform.rotate_y(rotation.unwrap_or_default().to_radians());
                if state.game_mode == GameMode::Soccar {
                    transform.translation.y += 2.6;
                } else if state.game_mode == GameMode::Hoops {
                    transform.translation.y += 5.2;
                }

                (large_pad_mesh.clone(), pad_glows.large_hitbox.clone())
            } else {
                if state.game_mode == GameMode::Soccar {
                    if transform.translation.z > 10. {
                        transform.rotate_y(PI);
                    }

                    if (1023f32..1025.).contains(&transform.translation.x.abs()) {
                        transform.rotate_y(PI / 6.);

                        if transform.translation.x > 1. {
                            transform.rotate_y(PI);
                        }
                    }

                    if (1023f32..1025.).contains(&transform.translation.z.abs()) {
                        transform.rotate_y(PI / 3.);
                    }

                    if (1787f32..1789.).contains(&transform.translation.x.abs())
                        && (2299f32..2301.).contains(&transform.translation.z.abs())
                    {
                        transform.rotate_y(PI.copysign(transform.translation.x * transform.translation.z) / 4.);
                    }
                } else if state.game_mode == GameMode::Hoops {
                    if transform.translation.z > 2810. {
                        transform.rotate_y(PI / 3.);
                    }

                    if (-2400f32..-2200.).contains(&transform.translation.z) {
                        transform.rotate_y(3. * PI.copysign(transform.translation.x) / 12.);
                    }

                    if (500f32..1537.).contains(&transform.translation.x.abs())
                        && (0f32..1025.).contains(&transform.translation.z)
                    {
                        transform.rotate_y(PI / 3.);
                    }

                    if (511f32..513.).contains(&transform.translation.x.abs())
                        && (511f32..513.).contains(&transform.translation.z.abs())
                    {
                        transform.rotate_y(PI.copysign(transform.translation.x * transform.translation.z) / 12.);
                    }

                    transform.translation.y += 5.7;
                }

                (small_pad_mesh.clone(), pad_glows.small_hitbox.clone())
            };

            commands
                .spawn((
                    BoostPadI(code),
                    PbrBundle {
                        mesh: visual_mesh,
                        transform,
                        material: materials.add(StandardMaterial {
                            base_color: Color::rgba(0.9, 0.9, 0.1, 0.6),
                            alpha_mode: AlphaMode::Add,
                            double_sided: true,
                            cull_mode: None,
                            ..default()
                        }),
                        ..default()
                    },
                    #[cfg(debug_assertions)]
                    EntityName::from("generic_boost_pad"),
                    RaycastPickable,
                    On::<Pointer<Over>>::target_insert(HighlightedEntity),
                    On::<Pointer<Out>>::target_remove::<HighlightedEntity>(),
                    On::<Pointer<Click>>::send_event::<BoostPadClicked>(),
                    NotShadowCaster,
                    NotShadowReceiver,
                ))
                .with_children(|parent| {
                    parent.spawn(PbrBundle {
                        mesh: hitbox,
                        material: hitbox_material.clone(),
                        ..default()
                    });
                });
        }
    }
}

fn update_pad_colors(
    state: Res<GameState>,
    query: Query<(&Handle<StandardMaterial>, &BoostPadI)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let morton_generator = Morton::default();

    let mut sorted_pads = state
        .pads
        .iter()
        .enumerate()
        .map(|(i, pad)| (i, morton_generator.get_code(pad.position)))
        .collect::<Vec<_>>();
    radsort::sort_by_key(&mut sorted_pads, |(_, code)| *code);

    for (handle, id) in query.iter() {
        let index = sorted_pads.binary_search_by_key(&id.id(), |(_, code)| *code).unwrap();
        let alpha = if state.pads[sorted_pads[index].0].state.is_active {
            0.6
        } else {
            // make the glow on inactive pads dissapear
            0.0
        };

        materials.get_mut(handle).unwrap().base_color.set_a(alpha);
    }
}

fn update_boost_meter(
    state: Res<GameState>,
    ui_scale: Res<UiOverlayScale>,
    camera: Query<&PrimaryCamera>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut painter: ShapePainter,
    mut boost_amount: Query<(&mut Text, &mut Style), With<BoostAmount>>,
    mut was_last_director: Local<bool>,
) {
    let id = match camera.single() {
        PrimaryCamera::Director(id) | PrimaryCamera::TrackCar(id) => *id,
        PrimaryCamera::Spectator => 0,
    };

    if id == 0 {
        if *was_last_director {
            *was_last_director = false;
            boost_amount.single_mut().0.sections[0].value.clear();
        }

        return;
    }

    let Some(car_state) = &state.cars.iter().find(|info| id == info.id).map(|info| info.state) else {
        return;
    };

    let primary_window = windows.single();
    let window_res = Vec2::new(primary_window.width(), primary_window.height());
    let painter_pos = (window_res / 2. - (BOOST_INDICATOR_POS + 25.) * ui_scale.scale) * Vec2::new(1., -1.);

    painter.set_translation(painter_pos.extend(0.));
    painter.color = Color::rgb(0.075, 0.075, 0.15);
    painter.circle(100.0 * ui_scale.scale);

    let scale = car_state.boost / 100.;

    let start_angle = 7. * PI / 6.;
    let full_angle = 11. * PI / 6.;
    let end_angle = (full_angle - start_angle).mul_add(scale, start_angle);

    painter.color = Color::rgb(1., 0.84 * scale, 0.);
    painter.hollow = true;
    painter.thickness = 4.;
    painter.arc(80. * ui_scale.scale, start_angle, end_angle);

    painter.reset();

    let (mut text_display, mut style) = boost_amount.single_mut();
    style.right = Val::Px((BOOST_INDICATOR_POS.x - 25.) * ui_scale.scale);
    style.bottom = Val::Px(BOOST_INDICATOR_POS.y * ui_scale.scale);

    text_display.sections[0].value = car_state.boost.round().to_string();
    text_display.sections[0].style.font_size = BOOST_INDICATOR_FONT_SIZE * ui_scale.scale;

    *was_last_director = true;
}

fn update_time(state: Res<GameState>, show_time: Res<ShowTime>, mut text_display: Query<&mut Text, With<TimeDisplay>>) {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    if !show_time.enabled {
        text_display.single_mut().sections[0].value = String::new();
        return;
    }

    let tick_rate = state.tick_rate.round() as u64;
    if tick_rate == 0 {
        return;
    }

    let mut seconds = state.tick_count / tick_rate;

    let mut time_segments = Vec::with_capacity(7);

    let years = seconds / YEAR;
    if years > 0 {
        time_segments.push(format!("{years}y"));
    }
    seconds -= years * YEAR;

    let months = seconds / MONTH;
    if months > 0 {
        time_segments.push(format!("{months:02}m"));
    }
    seconds -= months * MONTH;

    let weeks = seconds / WEEK;
    if weeks > 0 {
        time_segments.push(format!("{weeks:02}w"));
    }
    seconds -= weeks * WEEK;

    let days = seconds / DAY;
    if days > 0 {
        time_segments.push(format!("{days}d"));
    }
    seconds -= days * DAY;

    let hours = seconds / HOUR;
    if hours > 0 {
        time_segments.push(format!("{hours:02}h"));
    }
    seconds -= hours * HOUR;

    let minutes = seconds / MINUTE;
    time_segments.push(format!("{minutes:02}m"));
    seconds -= minutes * MINUTE;

    time_segments.push(format!("{seconds:02}s"));

    text_display.single_mut().sections[0].value = time_segments.join(":");
}

fn update_field(state: Res<GameState>, mut game_mode: ResMut<GameMode>, mut load_state: ResMut<NextState<GameLoadState>>) {
    if state.game_mode != *game_mode {
        *game_mode = state.game_mode;
        load_state.set(GameLoadState::Despawn);
    }
}

fn update_ball_rotation(
    mut state: ResMut<GameState>,
    extrapolation: Res<Extrapolation>,
    game_speed: Res<GameSpeed>,
    time: Res<Time>,
    mut last_game_tick: Local<u64>,
) {
    if game_speed.paused {
        return;
    }

    let delta_time = if extrapolation.0 {
        time.delta_seconds() * game_speed.speed
    } else {
        (state.tick_count - *last_game_tick) as f32 / state.tick_rate
    };

    *last_game_tick = state.tick_count;

    let ball_ang_vel = state.ball.ang_vel * delta_time;
    let ang_vel = ball_ang_vel.length();
    if ang_vel > f32::EPSILON {
        let axis = ball_ang_vel / ang_vel;
        let rot = Mat3A::from_axis_angle(axis.into(), ang_vel);
        state.ball.rot_mat = rot * state.ball.rot_mat;
    }
}

fn interpolate_packet(mut state: ResMut<GameState>, game_speed: Res<GameSpeed>, time: Res<Time>) {
    if game_speed.paused {
        return;
    }

    let delta_time = time.delta_seconds() * game_speed.speed;

    let ball_pos = state.ball.vel * delta_time;
    state.ball.pos += ball_pos;

    for car in state.cars.iter_mut() {
        let car_pos = car.state.vel * delta_time;
        car.state.pos += car_pos;

        let car_ang_vel = car.state.ang_vel * delta_time;
        let ang_vel = car_ang_vel.length();
        if ang_vel > f32::EPSILON {
            let axis = car_ang_vel / ang_vel;
            let rot = Mat3A::from_axis_angle(axis.into(), ang_vel);
            car.state.rot_mat = rot * car.state.rot_mat;
        }
    }
}

fn listen(socket: Res<Connection>, key: Res<ButtonInput<KeyCode>>, mut game_state: ResMut<GameState>) {
    let mut changed = false;
    if key.just_pressed(KeyCode::KeyR) {
        changed = true;

        game_state.ball.pos = Vec3A::new(0., -2000., 1500.);
        game_state.ball.vel = Vec3A::new(50., 1500., 1.);
    }

    if changed {
        if let Err(e) = socket.0.send_to(&game_state.to_bytes(), socket.1) {
            error!("Failed to send state setting packet: {e}");
        }
    }
}

#[derive(Resource)]
struct PacketUpdated(bool);

pub struct RocketSimPlugin;

impl Plugin for RocketSimPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PausedUpdate>()
            .add_event::<SpeedUpdate>()
            .insert_resource(GameState::default())
            .insert_resource(DirectorTimer(Timer::new(Duration::from_secs(12), TimerMode::Repeating)))
            .insert_resource(PacketUpdated(false))
            .insert_resource(GameMode::default())
            .add_plugins(UdpRendererPlugin)
            .add_systems(
                Update,
                (
                    establish_connection.run_if(in_state(GameLoadState::Connect)),
                    (
                        (
                            handle_udp,
                            (
                                (
                                    (
                                        update_ball_rotation.run_if(|calc_ball_rot: Res<CalcBallRot>| calc_ball_rot.0),
                                        update_ball,
                                    )
                                        .chain(),
                                    (
                                        pre_update_car,
                                        (update_car, update_car_extra, update_car_wheels),
                                        post_update_car,
                                    )
                                        .chain(),
                                    (update_pads_count, update_pad_colors).chain(),
                                    update_field,
                                )
                                    .run_if(|updated: Res<PacketUpdated>| updated.0),
                                (
                                    (interpolate_packet, update_ball_rotation),
                                    (update_ball, (update_car, post_update_car).chain(), update_car_wheels),
                                )
                                    .chain()
                                    .run_if(|updated: Res<PacketUpdated>, ip: Res<Extrapolation>| !updated.0 && ip.0),
                                (listen, update_boost_meter),
                            ),
                        )
                            .chain(),
                        update_time,
                    )
                        .run_if(in_state(GameLoadState::None)),
                ),
            );
    }
}
