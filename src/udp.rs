use crate::{
    GameLoadState, ServerPort,
    assets::{BoostPickupGlows, CarWheelMesh, get_material, get_mesh_info},
    bytes::{FromBytes, ToBytes, ToBytesExact},
    camera::{PrimaryCamera, TimeDisplay},
    mesh::LargeBoostPadLocRots,
    renderer::{RenderGroups, RenderMessage, UdpRendererPlugin},
    rocketsim::{CarInfo, GameMode, GameState, Team, TileState},
    settings::options::{BallCam, CalcBallRot, GameSpeed, Options, PacketSmoothing, ShowTime},
};
use ahash::AHashMap;
use bevy::{
    app::AppExit,
    asset::LoadState,
    color::palettes::css,
    light::{NotShadowCaster, NotShadowReceiver},
    math::{Mat3A, Vec3A},
    picking::mesh_picking::ray_cast::SimplifiedMesh,
    prelude::*,
    render::renderer::RenderDevice,
    time::Stopwatch,
};
use crossbeam_channel::{Receiver, Sender};
use itertools::izip;
use std::{
    f32::consts::PI,
    fs,
    mem::{replace, swap},
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    thread,
    time::Duration,
};

use crate::{
    camera::{BOOST_INDICATOR_FONT_SIZE, BOOST_INDICATOR_POS, BoostAmount, HighlightedEntity},
    mesh::{BoostPadClicked, CarClicked, ChangeCarPos},
    settings::{options::UiOverlayScale, state_setting::UserCarStates},
};
use bevy::window::PrimaryWindow;
use bevy_vector_shapes::prelude::*;

#[cfg(debug_assertions)]
use crate::camera::EntityName;

#[derive(Component)]
#[require(Mesh3d, MeshMaterial3d<StandardMaterial>, NotShadowCaster, NotShadowReceiver)]
pub struct BoostPadI(usize);

impl BoostPadI {
    #[inline]
    pub const fn idx(&self) -> usize {
        self.0
    }
}

#[derive(Component)]
#[require(Mesh3d, MeshMaterial3d<StandardMaterial>)]
pub struct Ball;

#[derive(Component)]
#[require(Mesh3d, MeshMaterial3d<StandardMaterial>)]
pub struct Car(u32);

impl Car {
    #[inline]
    pub const fn id(&self) -> u32 {
        self.0
    }
}

#[derive(Component)]
#[require(Mesh3d, MeshMaterial3d<StandardMaterial>)]
pub struct CarBody;

#[derive(Resource)]
struct DirectorTimer(Timer);

#[derive(Resource, Deref)]
pub struct Connection(Sender<SendableUdp>);

pub enum SendableUdp {
    Paused(bool),
    Speed(f32),
    State(GameState),
}

fn establish_connection(port: Res<ServerPort>, mut commands: Commands, mut state: ResMut<NextState<GameLoadState>>) {
    let out_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port.primary_port);
    let recv_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port.secondary_port);
    let socket = UdpSocket::bind(recv_addr).unwrap();

    let (tx, rx) = crossbeam_channel::unbounded();
    commands.insert_resource(Connection(tx));

    start_udp_recv_handler(socket.try_clone().unwrap(), &mut commands);
    start_udp_send_handler(socket, out_addr, rx);

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
        Self::new(self.x, self.z, self.y)
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

pub const BLUE_COLOR: Srgba = if cfg!(feature = "full_load") {
    Srgba::rgb(0.03, 0.09, 0.79)
} else {
    Srgba::rgb(0.01, 0.03, 0.39)
};

pub const ORANGE_COLOR: Srgba = if cfg!(feature = "full_load") {
    Srgba::rgb(0.41, 0.21, 0.01)
} else {
    Srgba::rgb(0.82, 0.42, 0.02)
};

#[inline]
/// Use colors that are a bit darker if we don't have the `full_load` feature
const fn get_color_from_team(team: Team) -> Color {
    match team {
        Team::Blue => Color::Srgba(BLUE_COLOR),
        Team::Orange => Color::Srgba(ORANGE_COLOR),
    }
}

#[derive(Component)]
#[require(Mesh3d, MeshMaterial3d<StandardMaterial>)]
pub struct CarBoost;

#[derive(Component)]
#[require(Mesh3d, MeshMaterial3d<StandardMaterial>)]
struct CarWheel {
    front: bool,
    left: bool,
}

impl CarWheel {
    const fn new(front: bool, left: bool) -> Self {
        Self { front, left }
    }
}

pub fn target_insert<M: EntityEvent>(component: impl Component + Clone) -> impl Fn(On<M>, Commands) {
    move |event, mut commands| {
        let entity = event.event().event_target();
        commands.entity(entity).insert(component.clone());
    }
}

pub fn target_remove<M: EntityEvent, C: Component>(event: On<M>, mut commands: Commands) {
    let entity = event.event().event_target();
    commands.entity(entity).remove::<C>();
}

pub fn write_message<M: EntityEvent, S: Message + for<'a> From<&'a M>>(event: On<M>, mut events: MessageWriter<S>) {
    events.write(S::from(event.event()));
}

fn spawn_car(
    car_info: &CarInfo,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    car_wheel_mesh: &CarWheelMesh,
    images: &mut Assets<Image>,
    render_device: Option<&RenderDevice>,
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
            SimplifiedMesh(meshes.add(Sphere::new(hitbox.y * 2.5))),
            Mesh3d(meshes.add(Cuboid::new(hitbox.x, hitbox.y, hitbox.z))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::NONE,
                alpha_mode: AlphaMode::Add,
                unlit: true,
                ..default()
            })),
            #[cfg(debug_assertions)]
            EntityName::from(name),
            Pickable::default(),
        ))
        .observe(target_insert::<Pointer<Over>>(HighlightedEntity))
        .observe(target_remove::<Pointer<Out>, HighlightedEntity>)
        .observe(write_message::<Pointer<Drag>, ChangeCarPos>)
        .observe(write_message::<Pointer<Click>, CarClicked>)
        .with_children(|parent| {
            const CAR_BOOST_LENGTH: f32 = 50.;

            let wheel_material = materials.add(StandardMaterial {
                base_color,
                perceptual_roughness: 0.7,
                ..default()
            });

            if cfg!(feature = "full_load") {
                let mesh_materials = get_car_mesh_materials(
                    mesh_id,
                    materials,
                    asset_server,
                    base_color,
                    car_info.team,
                    images,
                    render_device,
                );

                for (mesh, material) in mesh_info.into_iter().zip(mesh_materials) {
                    parent.spawn((CarBody, Mesh3d(mesh), MeshMaterial3d(material)));
                }
            } else {
                for mesh in mesh_info {
                    parent.spawn((CarBody, Mesh3d(mesh), MeshMaterial3d(wheel_material.clone())));
                }
            }

            parent.spawn((
                Mesh3d(meshes.add(Cylinder::new(10., CAR_BOOST_LENGTH))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgba(1., 1., 0., 0.),
                    alpha_mode: AlphaMode::Add,
                    cull_mode: None,
                    ..default()
                })),
                Transform {
                    translation: Vec3::new((hitbox.x + CAR_BOOST_LENGTH) / -2., hitbox.y / 2., 0.),
                    rotation: Quat::from_rotation_z(PI / 2.),
                    ..default()
                },
                NotShadowCaster,
                NotShadowReceiver,
                CarBoost,
            ));

            let wheel_pairs = [car_info.config.front_wheels, car_info.config.back_wheels];

            for (i, wheel_pair) in wheel_pairs.iter().enumerate() {
                let wheel_offset = -Vec3::Y * (wheel_pair.suspension_rest_length - 12.);

                for side in 0..=1 {
                    let fside = side as f32;
                    let offset = Vec3::new(1., 1., -2.0f32 * fside + 1.);

                    parent.spawn((
                        Mesh3d(car_wheel_mesh.mesh.clone()),
                        MeshMaterial3d(wheel_material.clone()),
                        Transform {
                            translation: wheel_pair.connection_point_offset.to_bevy() * offset + wheel_offset,
                            rotation: Quat::from_rotation_x(PI * fside),
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
    side: Team,
    images: &mut Assets<Image>,
    render_device: Option<&RenderDevice>,
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

        mesh_materials.push(get_material(
            material_name,
            materials,
            asset_server,
            Some(base_color),
            Some(side),
            images,
            render_device,
        ));
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

#[derive(Message)]
pub struct SpeedUpdate(pub f32);

#[derive(Message)]
pub struct PausedUpdate(pub bool);

enum UdpUpdate {
    State(GameState),
    Render(RenderMessage),
    Speed(f32),
    Paused(bool),
    Connection,
    Exit,
}

#[derive(Resource, Deref)]
struct UdpUpdateStream(Receiver<UdpUpdate>);

fn start_udp_send_handler(socket: UdpSocket, out_addr: SocketAddr, outgoing: Receiver<SendableUdp>) {
    socket.send_to(&[UdpPacketTypes::Connection as u8], out_addr).unwrap();

    thread::spawn(move || {
        loop {
            match outgoing.recv() {
                Ok(SendableUdp::State(state)) => {
                    let bytes = state.to_bytes();

                    if socket.send_to(&[UdpPacketTypes::GameState as u8], out_addr).is_err() {
                        continue;
                    }

                    if socket.send_to(&bytes, out_addr).is_err() {
                        continue;
                    }
                }
                Ok(SendableUdp::Speed(speed)) => {
                    let bytes = speed.to_bytes();

                    if socket.send_to(&[UdpPacketTypes::Speed as u8], out_addr).is_err() {
                        continue;
                    }

                    if socket.send_to(&bytes, out_addr).is_err() {
                        continue;
                    }
                }
                Ok(SendableUdp::Paused(paused)) => {
                    let paused = [paused as u8];

                    if socket.send_to(&[UdpPacketTypes::Paused as u8], out_addr).is_err() {
                        continue;
                    }

                    if socket.send_to(&paused, out_addr).is_err() {
                        continue;
                    }
                }
                Err(_) => return,
            }
        }
    });
}

fn start_udp_recv_handler(socket: UdpSocket, commands: &mut Commands) {
    let (tx, rx) = crossbeam_channel::unbounded();

    thread::spawn(move || {
        let mut packet_type_buffer = [0];
        let mut initial_state_buffer = [0; GameState::MIN_NUM_BYTES];
        let mut initial_render_buffer = [0; RenderMessage::MIN_NUM_BYTES];
        let mut speed_buffer = [0; 4];
        let mut paused_buffer = [0];

        let mut buf = Vec::new();
        let mut render_buf = Vec::new();
        let mut last_game_state = GameState::default();

        loop {
            if socket.recv_from(&mut packet_type_buffer).is_err() {
                return;
            }

            let Some(packet_type) = UdpPacketTypes::new(packet_type_buffer[0]) else {
                return;
            };

            match packet_type {
                UdpPacketTypes::Quit => {
                    drop(tx.send(UdpUpdate::Exit));
                    return;
                }
                UdpPacketTypes::GameState => {
                    // wait until we receive the packet
                    // it should arrive VERY quickly, so a loop with no delay is fine
                    // if it doesn't, then there are other problems lol
                    // UPDATE: Windows throws a specific error that we need to look for
                    // despite the fact that it actually worked

                    #[cfg(windows)]
                    {
                        while let Err(e) = socket.peek_from(&mut initial_state_buffer) {
                            if let Some(code) = e.raw_os_error() {
                                if code == 10040 {
                                    break;
                                }
                            }
                        }
                    }

                    #[cfg(not(windows))]
                    {
                        while socket.peek_from(&mut initial_state_buffer).is_err() {}
                    }

                    let new_tick_count = GameState::read_tick_count(&initial_state_buffer);
                    if new_tick_count > 15 && last_game_state.tick_count > new_tick_count {
                        drop(socket.recv_from(&mut [0]));
                        return;
                    }

                    buf.resize(GameState::get_num_bytes(&initial_state_buffer), 0);
                    if socket.recv_from(&mut buf).is_err() {
                        return;
                    }

                    last_game_state = GameState::from_bytes(&buf);
                    if tx.send(UdpUpdate::State(last_game_state.clone())).is_err() {
                        return;
                    }
                }
                UdpPacketTypes::Render => {
                    #[cfg(windows)]
                    {
                        while let Err(e) = socket.peek_from(&mut initial_render_buffer) {
                            if let Some(code) = e.raw_os_error() {
                                if code == 10040 {
                                    break;
                                }
                            }
                        }
                    }

                    #[cfg(not(windows))]
                    {
                        while socket.peek_from(&mut initial_render_buffer).is_err() {}
                    }

                    render_buf.resize(RenderMessage::get_num_bytes(&initial_render_buffer), 0);
                    if socket.recv_from(&mut render_buf).is_err() {
                        return;
                    }

                    let render_message = RenderMessage::from_bytes(&render_buf);
                    if tx.send(UdpUpdate::Render(render_message)).is_err() {
                        return;
                    }
                }
                UdpPacketTypes::Speed => {
                    if socket.recv_from(&mut speed_buffer).is_err() {
                        return;
                    }

                    let speed = f32::from_le_bytes(speed_buffer);
                    if tx.send(UdpUpdate::Speed(speed)).is_err() {
                        return;
                    }
                }
                UdpPacketTypes::Paused => {
                    if socket.recv_from(&mut paused_buffer).is_err() {
                        return;
                    }

                    let paused = paused_buffer[0] != 0;
                    if tx.send(UdpUpdate::Paused(paused)).is_err() {
                        return;
                    }
                }
                UdpPacketTypes::Connection => {
                    if tx.send(UdpUpdate::Connection).is_err() {
                        return;
                    }
                }
            }
        }
    });

    commands.insert_resource(UdpUpdateStream(rx));
}

fn apply_udp_updates(
    time: Res<Time>,
    socket: Res<Connection>,
    udp_updates: Res<UdpUpdateStream>,
    game_speed: Res<GameSpeed>,
    calc_ball_rot: Res<CalcBallRot>,
    packet_smoothing: Res<PacketSmoothing>,
    mut game_states: ResMut<GameStates>,
    mut exit: MessageWriter<AppExit>,
    mut packet_updated: ResMut<PacketUpdated>,
    mut render_groups: ResMut<RenderGroups>,
    mut packet_time_elapsed: ResMut<PacketTimeElapsed>,
    mut last_packet_time_elapsed: ResMut<LastPacketTimesElapsed>,
    mut speed_update: MessageWriter<SpeedUpdate>,
    mut paused_update: MessageWriter<PausedUpdate>,
) {
    packet_time_elapsed.tick(time.delta());

    let mut new_game_state = None;

    for update in udp_updates.try_iter() {
        match update {
            UdpUpdate::Exit => {
                exit.write(AppExit::Success);
                return;
            }
            UdpUpdate::State(new_state) => {
                new_game_state = Some(new_state);
            }
            UdpUpdate::Render(render_message) => match render_message {
                RenderMessage::AddRender(group_id, renders) => {
                    render_groups.groups.insert(group_id, renders);
                }
                RenderMessage::RemoveRender(group_id) => {
                    render_groups.groups.remove(&group_id);
                }
            },
            UdpUpdate::Speed(speed) => {
                last_packet_time_elapsed.reset();
                speed_update.write(SpeedUpdate(speed));
            }
            UdpUpdate::Paused(paused) => {
                paused_update.write(PausedUpdate(paused));
            }
            UdpUpdate::Connection => {
                socket.send(SendableUdp::Paused(game_speed.paused)).unwrap();
                socket.send(SendableUdp::Speed(game_speed.speed)).unwrap();
            }
        }
    }

    match new_game_state {
        Some(new_state) => {
            last_packet_time_elapsed.push(packet_time_elapsed.0.elapsed_secs());
            packet_time_elapsed.reset();

            game_states.advance(*packet_smoothing, new_state, calc_ball_rot.0);
            packet_updated.0 = true;
        }
        None => {
            packet_updated.0 = false;
        }
    }
}

fn update_ball(
    states: Res<GameStates>,
    mut ball: Query<(&mut Transform, &Children), With<Ball>>,
    mut point_light: Query<&mut PointLight>,
) {
    let Ok((mut transform, children)) = ball.single_mut() else {
        return;
    };

    let new_pos = states.current.ball.pos.to_bevy();
    transform.translation = new_pos;

    for child in children {
        let Ok(mut point_light) = point_light.get_mut(*child) else {
            continue;
        };

        let amount = (transform.translation.z.abs() + 500.) / 3500.;
        point_light.color = if new_pos.z > 0. {
            Color::srgb(amount.max(0.5), (amount * (2. / 3.)).max(0.5), 0.5)
        } else {
            Color::srgb(0.5, 0.5, amount.max(0.5))
        };

        transform.rotation = states.current.ball.rot_mat.to_bevy();

        break;
    }
}

const MIN_CAMERA_BALLCAM_HEIGHT: f32 = 30.;

fn update_car(states: Res<GameStates>, mut cars: Query<(&mut Transform, &Car)>) {
    for (mut car_transform, car) in &mut cars {
        let Some(target_car) = states.current.cars.iter().find(|car_info| car.0 == car_info.id) else {
            continue;
        };

        car_transform.translation = target_car.state.pos.to_bevy();
        car_transform.rotation = target_car.state.rot_mat.to_bevy();
    }
}

fn update_car_extra(
    states: Res<GameStates>,
    mut cars: Query<(&Car, &Children)>,
    mut car_boosts: Query<&MeshMaterial3d<StandardMaterial>, With<CarBoost>>,
    mut car_materials: Query<&MeshMaterial3d<StandardMaterial>, With<CarBody>>,
    mut car_wheels: Query<&MeshMaterial3d<StandardMaterial>, With<CarWheel>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last_boost_states: Local<Vec<u32>>,
    mut last_demoed_states: Local<Vec<u32>>,
    mut last_boost_amounts: Local<AHashMap<u32, f32>>,
) {
    for (car, children) in &mut cars {
        let Some(target_car) = states.current.cars.iter().find(|car_info| car.0 == car_info.id) else {
            continue;
        };

        let is_demoed = target_car.state.is_demoed || target_car.state.demo_respawn_timer > f32::EPSILON;
        let last_demoed = last_demoed_states.iter().any(|&id| id == car.id());

        if is_demoed != last_demoed {
            for child in children {
                let Ok(material_handle) = car_materials.get_mut(*child) else {
                    continue;
                };

                let material = materials.get_mut(material_handle).unwrap();
                if is_demoed {
                    material.base_color.set_alpha(0.);
                    material.alpha_mode = AlphaMode::Add;
                    last_demoed_states.push(car.id());
                } else {
                    material.alpha_mode = AlphaMode::Opaque;
                    last_demoed_states.retain(|&id| id != car.id());
                }
            }

            for child in children {
                let Ok(material_handle) = car_wheels.get_mut(*child) else {
                    continue;
                };

                let material = materials.get_mut(material_handle).unwrap();
                if is_demoed {
                    material.base_color.set_alpha(0.);
                    material.alpha_mode = AlphaMode::Add;
                    last_demoed_states.push(car.id());
                } else {
                    material.alpha_mode = AlphaMode::Opaque;
                    last_demoed_states.retain(|&id| id != car.id());
                }
            }
        }

        let last_boost_amount = last_boost_amounts
            .insert(car.id(), target_car.state.boost)
            .unwrap_or_default();

        let is_boosting = !is_demoed
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
                    material.base_color.set_alpha(0.7);
                    last_boost_states.push(car.id());
                } else {
                    material.base_color.set_alpha(0.0);
                    last_boost_states.retain(|&id| id != car.id());
                }

                break;
            }
        }
    }
}

fn update_car_wheels(
    states: Res<GameStates>,
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
        game_speed.speed / states.current.tick_rate
    } else {
        time.delta_secs() * game_speed.speed
    };

    calc_car_wheel_update(&states.current, cars, car_wheels, delta_time);
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
    states: Res<GameStates>,
    asset_server: Res<AssetServer>,
    car_entities: Query<(Entity, &Car)>,
    commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut user_cars: ResMut<UserCarStates>,
    car_wheel_mesh: Res<CarWheelMesh>,
    mut images: ResMut<Assets<Image>>,
    render_device: Option<Res<RenderDevice>>,
    mut prev_tick_count: Local<u64>,
) {
    if *prev_tick_count == states.current.tick_count {
        return;
    }

    *prev_tick_count = states.current.tick_count;
    correct_car_count(
        &cars,
        &states.current,
        &car_entities,
        &mut user_cars,
        commands,
        &mut meshes,
        &mut materials,
        &asset_server,
        &car_wheel_mesh,
        &mut images,
        render_device.as_deref(),
    );
}

fn update_camera(
    time: Res<Time>,
    states: Res<GameStates>,
    ballcam: Res<BallCam>,
    mut cars: Query<(&mut Transform, &Car)>,
    mut camera_query: Query<(&mut PrimaryCamera, &mut Transform), Without<Car>>,
    mut timer: ResMut<DirectorTimer>,
) {
    timer.0.tick(time.delta());

    let (mut primary_camera, mut camera_transform) = camera_query.single_mut().unwrap();

    let car_id = match primary_camera.as_mut() {
        PrimaryCamera::TrackCar(id) => {
            if states.current.cars.is_empty() {
                return;
            }

            let mut ids = states.current.cars.iter().map(|car_info| car_info.id).collect::<Vec<_>>();
            ids.sort();

            let index = *id as usize - 1;
            if index >= ids.len() {
                return;
            }

            ids[index]
        }
        PrimaryCamera::Director(id) => {
            if *id == 0 || timer.0.is_finished() {
                // get the car closest to the ball
                let mut min_dist = f32::MAX;
                let mut new_id = *id;
                for car in &*states.current.cars {
                    let dist = car.state.pos.distance_squared(states.current.ball.pos);
                    if dist < min_dist {
                        new_id = car.id;
                        min_dist = dist;
                    }
                }

                *id = new_id;
            }

            *id
        }
        PrimaryCamera::Spectator => return,
    };

    let Some((car_transform, _)) = cars.iter_mut().find(|(_, car)| car.id() == car_id) else {
        return;
    };

    let Some(target_car) = states.current.cars.iter().find(|car_info| car_id == car_info.id) else {
        return;
    };

    let camera_transform = camera_transform.as_mut();

    if ballcam.enabled {
        let ball_pos = states.current.ball.pos.to_bevy();
        camera_transform.translation = car_transform.translation + (car_transform.translation - ball_pos).normalize() * 300.;
        camera_transform.look_at(ball_pos, Vec3::Y);
        camera_transform.translation += camera_transform.up() * 150.;
        camera_transform.look_at(ball_pos, Vec3::Y);
        camera_transform.translation.y = camera_transform.translation.y.max(MIN_CAMERA_BALLCAM_HEIGHT);
    } else {
        let car_look = Vec3::new(target_car.state.vel.x, 0., target_car.state.vel.y)
            .try_normalize()
            .unwrap_or_else(|| car_transform.forward().into());
        camera_transform.translation = car_transform.translation - car_look * 280. + Vec3::Y * 110.;
        camera_transform.look_to(car_look, Vec3::Y);
        camera_transform.rotation *= Quat::from_rotation_x(-PI / 30.);
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
    images: &mut Assets<Image>,
    render_device: Option<&RenderDevice>,
) {
    // remove cars that no longer exist
    for (entity, car) in car_entities {
        if !state.cars.iter().any(|car_info| car.0 == car_info.id) {
            user_cars.remove(car.0);
            commands.entity(entity).despawn();
        }
    }

    // add new cars
    let non_existant_cars = state
        .cars
        .iter()
        .filter(|car_info| !cars.iter().any(|id| id.0 == car_info.id));

    for car_info in non_existant_cars {
        spawn_car(
            car_info,
            &mut commands,
            meshes,
            materials,
            asset_server,
            car_wheel_mesh,
            images,
            render_device,
        );
    }
}

fn update_pads_count(
    states: Res<GameStates>,
    asset_server: Res<AssetServer>,
    pads: Query<(Entity, &BoostPadI)>,
    pad_glows: Res<BoostPickupGlows>,
    large_boost_pad_loc_rots: Res<LargeBoostPadLocRots>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    if pads.iter().count() == states.current.pads.len() || large_boost_pad_loc_rots.rots.is_empty() {
        return;
    }

    // The number of pads shouldn't change often
    // There's also not an easy way to determine
    // if a previous pad a new pad are same pad
    // It is the easiest to despawn and respawn all pads
    for (entity, _) in pads.iter() {
        commands.entity(entity).despawn();
    }

    let large_pad_mesh = match asset_server.get_load_state(&pad_glows.large) {
        Some(LoadState::Failed(_)) => pad_glows.large_hitbox.clone(),
        _ => pad_glows.large.clone(),
    };

    let small_pad_mesh = match asset_server.get_load_state(&pad_glows.small) {
        Some(LoadState::Failed(_)) => pad_glows.small_hitbox.clone(),
        _ => pad_glows.small.clone(),
    };

    for (i, pad) in states.current.pads.iter().enumerate() {
        let mut transform = Transform::from_translation(pad.position.to_bevy() - Vec3::Y * 70.);

        let (visual_mesh, hitbox) = if pad.is_big {
            let rotation = large_boost_pad_loc_rots
                .locs
                .iter()
                .enumerate()
                .find(|(_, loc)| loc.distance_squared(pad.position.xy()) < 25.)
                .map(|(i, _)| large_boost_pad_loc_rots.rots[i]);
            transform.rotate_y(rotation.unwrap_or_default().to_radians());
            if states.current.game_mode == GameMode::Soccar {
                transform.translation.y += 2.6;
            } else if states.current.game_mode == GameMode::Hoops {
                transform.translation.y += 5.2;
            }

            (large_pad_mesh.clone(), pad_glows.large_hitbox.clone())
        } else {
            if states.current.game_mode == GameMode::Soccar {
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
            } else if states.current.game_mode == GameMode::Hoops {
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
                BoostPadI(i),
                SimplifiedMesh(hitbox),
                Mesh3d(visual_mesh),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgba(0.9, 0.9, 0.1, 0.6),
                    alpha_mode: AlphaMode::Add,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                })),
                NotShadowCaster,
                NotShadowReceiver,
                transform,
                #[cfg(debug_assertions)]
                EntityName::from("generic_boost_pad"),
                Pickable::default(),
            ))
            .observe(target_insert::<Pointer<Over>>(HighlightedEntity))
            .observe(target_remove::<Pointer<Out>, HighlightedEntity>)
            .observe(write_message::<Pointer<Click>, BoostPadClicked>);
    }
}

fn update_pad_colors(
    states: Res<GameStates>,
    query: Query<(&BoostPadI, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut prev_tick_count: Local<u64>,
) {
    if *prev_tick_count == states.current.tick_count {
        return;
    }

    *prev_tick_count = states.current.tick_count;
    for (pad, material) in query.iter() {
        let new_alpha = if states.current.pads[pad.idx()].state.is_active {
            0.6
        } else {
            // make the glow on inactive pads disappear
            0.0
        };

        let current_alpha = materials.get(material).unwrap().base_color.alpha();
        if current_alpha != new_alpha {
            materials.get_mut(material).unwrap().base_color.set_alpha(new_alpha);
        }
    }
}

fn update_boost_meter(
    states: Res<GameStates>,
    ui_scale: Res<UiOverlayScale>,
    camera: Query<&PrimaryCamera>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut painter: ShapePainter,
    mut boost_amount: Query<(&mut Text, &mut Node, &mut TextFont), With<BoostAmount>>,
    mut was_last_director: Local<bool>,
) {
    const START_ANGLE: f32 = 7. * PI / 6.;
    const FULL_ANGLE: f32 = 11. * PI / 6.;
    const UI_BACKGROUND: Color = Color::srgb(0.075, 0.075, 0.15);

    let id = match camera.single().unwrap() {
        PrimaryCamera::TrackCar(id) => {
            if states.current.cars.is_empty() {
                return;
            }

            let mut ids = states.current.cars.iter().map(|car_info| car_info.id).collect::<Vec<_>>();
            ids.sort();

            let index = *id as usize - 1;
            if index >= ids.len() { 0 } else { ids[index] }
        }
        PrimaryCamera::Director(id) => *id,
        PrimaryCamera::Spectator => 0,
    };

    if id == 0 {
        if *was_last_director {
            *was_last_director = false;
            boost_amount.single_mut().unwrap().0.0.clear();
        }

        return;
    }

    let Some(car_state) = &states.current.cars.iter().find(|info| id == info.id).map(|info| info.state) else {
        return;
    };

    let primary_window = windows.single().unwrap();
    let window_res = Vec2::new(primary_window.width(), primary_window.height());
    let painter_pos = (window_res / 2. - (BOOST_INDICATOR_POS + 25.) * ui_scale.scale) * Vec2::new(1., -1.);

    painter.set_translation(painter_pos.extend(0.));
    painter.color = UI_BACKGROUND;
    painter.circle(100.0 * ui_scale.scale);

    let scale = car_state.boost / 100.;
    let end_angle = (FULL_ANGLE - START_ANGLE) * scale + START_ANGLE;

    painter.color = Color::srgb(1., 0.84 * scale, 0.);
    painter.hollow = true;
    painter.thickness = 4.;
    painter.arc(80. * ui_scale.scale, START_ANGLE, end_angle);

    painter.reset();

    let (mut text_display, mut style, mut font) = boost_amount.single_mut().unwrap();
    style.right = Val::Px((BOOST_INDICATOR_POS.x - 25.) * ui_scale.scale);
    style.bottom = Val::Px(BOOST_INDICATOR_POS.y * ui_scale.scale);

    let boost_val = car_state.boost.round() as u8;

    text_display.clear();
    text_display.push_str(itoa::Buffer::new().format(boost_val));
    font.font_size = BOOST_INDICATOR_FONT_SIZE * ui_scale.scale;

    *was_last_director = true;
}

fn update_time(
    states: Res<GameStates>,
    show_time: Res<ShowTime>,
    mut text_display: Query<&mut Text, With<TimeDisplay>>,
    mut prev_tick_count: Local<u64>,
    mut prev_enabled: Local<bool>,
) {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    const OPTIONAL_TIME_SEGMENTS: [(u64, char, usize); 5] =
        [(YEAR, 'y', 0), (MONTH, 'm', 2), (WEEK, 'w', 2), (DAY, 'd', 0), (HOUR, 'h', 2)];

    const REQUIRED_TIME_SEGMENTS: [(u64, char, usize); 2] = [(MINUTE, 'm', 2), (1, 's', 2)];

    if *prev_tick_count == states.current.tick_count && show_time.enabled == *prev_enabled {
        return;
    }
    *prev_tick_count = states.current.tick_count;
    *prev_enabled = show_time.enabled;

    let text = &mut text_display.single_mut().unwrap().0;
    text.clear();

    if !show_time.enabled {
        return;
    }

    let tick_rate = states.current.tick_rate.round() as u64;
    if tick_rate == 0 {
        return;
    }

    let mut itoa_buf = itoa::Buffer::new();
    let mut seconds = states.current.tick_count / tick_rate;

    for (denom, unit, round) in OPTIONAL_TIME_SEGMENTS {
        let val = seconds / denom;
        if val > 0 {
            let val_str = itoa_buf.format(val);

            if !text.is_empty() {
                text.push(':');
            }

            if val_str.len() < round {
                let num_pad = round - val_str.len();
                for _ in 0..num_pad {
                    text.push('0');
                }
            }

            text.push_str(val_str);
            text.push(unit);
        }

        seconds -= val * denom;
    }

    for (denom, unit, round) in REQUIRED_TIME_SEGMENTS {
        if !text.is_empty() {
            text.push(':');
        }

        let val = seconds / denom;
        let val_str = itoa_buf.format(val);

        if val_str.len() < round {
            let num_pad = round - val_str.len();
            for _ in 0..num_pad {
                text.push('0');
            }
        }

        text.push_str(val_str);
        text.push(unit);

        seconds -= val * denom;
    }
}

fn update_field(states: Res<GameStates>, mut game_mode: ResMut<GameMode>, mut load_state: ResMut<NextState<GameLoadState>>) {
    if states.current.game_mode != *game_mode {
        *game_mode = states.current.game_mode;
        load_state.set(GameLoadState::Despawn);
    }
}

fn update_ball_rotation(
    mut states: ResMut<GameStates>,
    packet_smoothing: Res<PacketSmoothing>,
    game_speed: Res<GameSpeed>,
    time: Res<Time>,
    mut last_game_tick: Local<u64>,
) {
    if game_speed.paused {
        return;
    }

    if *last_game_tick > states.current.tick_count {
        *last_game_tick = states.current.tick_count;
    }

    let delta_time = if matches!(*packet_smoothing, PacketSmoothing::None) {
        (states.current.tick_count - *last_game_tick) as f32 / states.current.tick_rate
    } else {
        time.delta_secs() * game_speed.speed
    };

    *last_game_tick = states.current.tick_count;

    let ball_ang_vel = states.current.ball.ang_vel * delta_time;
    let ang_vel = ball_ang_vel.length();
    if ang_vel > f32::EPSILON {
        let axis = ball_ang_vel / ang_vel;
        let rot = Mat3A::from_axis_angle(axis.into(), ang_vel);
        states.current.ball.rot_mat = rot * states.current.ball.rot_mat;
    }
}

fn extrapolate_packet(mut states: ResMut<GameStates>, game_speed: Res<GameSpeed>, time: Res<Time>) {
    if game_speed.paused {
        return;
    }

    let delta_time = time.delta_secs() * game_speed.speed;

    let ball_pos = states.current.ball.vel * delta_time;
    states.current.ball.pos += ball_pos;

    for car in &mut states.current.cars {
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

fn interpolate_calc_next_ball_rot(mut states: ResMut<GameStates>) {
    states.current.ball.rot_mat = states.last.ball.rot_mat;

    if states.next.tick_count < states.last.tick_count {
        return;
    }

    let delta_time = (states.next.tick_count - states.last.tick_count) as f32 / states.next.tick_rate;

    let ball_ang_vel = states.last.ball.ang_vel * delta_time;
    let ang_vel = ball_ang_vel.length();
    if ang_vel > f32::EPSILON {
        let axis = ball_ang_vel / ang_vel;
        let rot = Mat3A::from_axis_angle(axis.into(), ang_vel);
        states.next.ball.rot_mat = rot * states.last.ball.rot_mat;
    }
}

fn interpolate_packets(
    time: Res<Time>,
    game_speed: Res<GameSpeed>,
    last_packet_time_elapsed: Res<LastPacketTimesElapsed>,
    mut states: ResMut<GameStates>,
    mut packet_time_elapsed: ResMut<PacketTimeElapsed>,
) {
    if game_speed.paused {
        return;
    }

    packet_time_elapsed.tick(time.delta());

    let delta_time = packet_time_elapsed.elapsed_secs();

    // Don't start extrapolating forever
    if delta_time > 2. {
        return;
    }

    let lerp_amount = delta_time / last_packet_time_elapsed.avg();

    states.current.ball.pos = states.last.ball.pos.lerp(states.next.ball.pos, lerp_amount);

    let last_ball_quat = Quat::from_mat3a(&states.last.ball.rot_mat);
    let next_ball_quat = Quat::from_mat3a(&states.next.ball.rot_mat);

    let curr_ball_quat = last_ball_quat.slerp(next_ball_quat, lerp_amount);
    states.current.ball.rot_mat = Mat3A::from_quat(curr_ball_quat);

    for (last_car, current_car, next_car) in states.iter_current_cars() {
        current_car.state.pos = last_car.state.pos.lerp(next_car.state.pos, lerp_amount);
        current_car.state.vel = last_car.state.vel.lerp(next_car.state.vel, lerp_amount);

        let last_car_quat = Quat::from_mat3a(&last_car.state.rot_mat);
        let next_car_quat = Quat::from_mat3a(&next_car.state.rot_mat);

        let curr_car_quat = last_car_quat.slerp(next_car_quat, lerp_amount);
        current_car.state.rot_mat = Mat3A::from_quat(curr_car_quat);
    }
}

fn listen(
    socket: Res<Connection>,
    key: Res<ButtonInput<KeyCode>>,
    mut game_states: ResMut<GameStates>,
    mut options: ResMut<Options>,
) {
    let mut changed = false;
    if key.just_pressed(KeyCode::KeyR) {
        changed = true;

        let pos = Vec3A::new(0., -2000., 1500.);
        let vel = Vec3A::new(50., 1500., 1.);

        game_states.current.ball.pos = pos;
        game_states.current.ball.vel = vel;
        game_states.next.ball.pos = pos;
        game_states.next.ball.vel = vel;
    }

    if key.just_pressed(KeyCode::KeyP) {
        options.paused = !options.paused;
    }

    let shift_pressed = key.pressed(KeyCode::ShiftLeft) || key.pressed(KeyCode::ShiftRight);

    if key.just_pressed(KeyCode::NumpadAdd) || (shift_pressed && key.just_pressed(KeyCode::Equal)) {
        options.game_speed = if options.game_speed < 0.5 {
            0.5
        } else {
            (options.game_speed + 0.5).min(10.)
        };
    }

    if key.just_pressed(KeyCode::NumpadSubtract) || (!shift_pressed && key.just_pressed(KeyCode::Minus)) {
        options.game_speed = (options.game_speed - 0.5).max(0.1);
    }

    if key.just_pressed(KeyCode::NumpadEqual) || (!shift_pressed && key.just_pressed(KeyCode::Equal)) {
        options.game_speed = 1.;
    }

    if changed {
        socket.send(SendableUdp::State(game_states.next.clone())).unwrap();
    }
}

#[derive(Resource, Default)]
struct PacketUpdated(bool);

#[derive(Resource, Default)]
pub struct GameStates {
    pub last: GameState,
    pub current: GameState,
    pub next: GameState,
}

impl GameStates {
    pub fn advance(&mut self, packet_smoothing: PacketSmoothing, new_state: GameState, calc_ball_rot: bool) {
        match packet_smoothing {
            PacketSmoothing::None | PacketSmoothing::Extrapolate => {
                self.last = replace(&mut self.next, new_state);

                if calc_ball_rot {
                    self.next.ball.rot_mat = self.current.ball.rot_mat;
                }

                self.current = self.next.clone();
            }
            PacketSmoothing::Interpolate => {
                swap(&mut self.last, &mut self.next);
                self.current = self.last.clone();
                self.next = new_state;
            }
        }
    }

    pub fn iter_current_cars(&mut self) -> impl Iterator<Item = (&CarInfo, &mut CarInfo, &CarInfo)> {
        izip!(self.last.cars.iter(), self.current.cars.iter_mut(), self.next.cars.iter())
    }
}

#[derive(Resource, Default, DerefMut, Deref)]
struct PacketTimeElapsed(Stopwatch);

#[derive(Resource, Default)]
pub struct LastPacketTimesElapsed {
    times: [f32; 15],
    start: usize,
    len: usize,
}

impl LastPacketTimesElapsed {
    fn push(&mut self, time: f32) {
        if self.len == self.times.len() {
            self.times[self.start] = time;
            self.start = (self.start + 1) % self.times.len();
        } else {
            self.times[self.len] = time;
            self.len += 1;
        }
    }

    pub fn reset(&mut self) {
        self.len = 0;
    }

    fn avg(&self) -> f32 {
        if self.len == 0 {
            return 1. / 120.;
        }

        let mut sum = 0.;
        for i in 0..self.len {
            sum += self.times[(self.start + i) % self.len];
        }
        sum / self.len as f32
    }
}

#[derive(Resource)]
struct TileInfo {
    pub state: TileState,
}

#[derive(Component)]
pub struct Tile {
    pub team: usize,
    pub index: usize,
}

pub fn get_tile_color(state: TileState) -> Color {
    match state {
        TileState::Full => css::GREEN,
        TileState::Damaged => css::RED,
        TileState::Broken => css::BLACK,
    }
    .into()
}

fn update_tiles(
    game_states: Res<GameStates>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tiles: Query<(&Tile, &MeshMaterial3d<StandardMaterial>)>,
    mut tile_states: Local<[Vec<TileInfo>; 2]>,
    mut prev_tick_count: Local<u64>,
) {
    if *prev_tick_count == game_states.current.tick_count {
        return;
    }

    *prev_tick_count = game_states.current.tick_count;
    if tile_states[0].len() != game_states.current.tiles[0].len() {
        for (sim_team_tiles, world_team_tiles) in game_states.current.tiles.iter().zip(&mut tile_states) {
            world_team_tiles.clear();
            for tile in sim_team_tiles.iter() {
                world_team_tiles.push(TileInfo { state: tile.state });
            }
        }
        return;
    }

    // check if the color needs to be updated because the state has changed
    for (tile, material) in &mut tiles {
        let proper_state = game_states.current.tiles[tile.team][tile.index].state;
        if proper_state != tile_states[tile.team][tile.index].state {
            tile_states[tile.team][tile.index].state = proper_state;
            let material = materials.get_mut(material).unwrap();
            material.base_color = get_tile_color(proper_state);
        }
    }
}

fn dropshot_update_ball(
    mut materials: ResMut<Assets<StandardMaterial>>,
    ball: Query<&MeshMaterial3d<StandardMaterial>, With<Ball>>,
    game_state: Res<GameStates>,
    mut y_target_dir: Local<f32>,
) {
    if (game_state.current.ball.ds_info.y_target_dir - *y_target_dir).abs() <= f32::EPSILON {
        return;
    }

    let material = materials.get_mut(ball.single().unwrap()).unwrap();
    *y_target_dir = game_state.current.ball.ds_info.y_target_dir;

    let base_color = if game_state.current.ball.ds_info.y_target_dir < 0.0 {
        css::RED
    } else if game_state.current.ball.ds_info.y_target_dir > 0.0 {
        css::BLUE
    } else {
        css::WHITE
    };

    material.base_color = base_color.into();
}

pub struct RocketSimPlugin;

impl Plugin for RocketSimPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PausedUpdate>()
            .add_message::<SpeedUpdate>()
            .insert_resource(GameStates::default())
            .insert_resource(DirectorTimer(Timer::new(Duration::from_secs(12), TimerMode::Repeating)))
            .insert_resource(PacketTimeElapsed::default())
            .insert_resource(LastPacketTimesElapsed::default())
            .insert_resource(PacketUpdated::default())
            .insert_resource(GameMode::default())
            .add_plugins(UdpRendererPlugin)
            .add_systems(
                Update,
                (
                    establish_connection.run_if(in_state(GameLoadState::Connect)),
                    (
                        (
                            apply_udp_updates,
                            (
                                (
                                    (
                                        (
                                            (
                                                interpolate_calc_next_ball_rot.run_if(|ps: Res<PacketSmoothing>| {
                                                    matches!(*ps, PacketSmoothing::Interpolate)
                                                }),
                                                update_ball_rotation.run_if(|ps: Res<PacketSmoothing>| {
                                                    !matches!(*ps, PacketSmoothing::Interpolate)
                                                }),
                                            )
                                                .run_if(|calc_ball_rot: Res<CalcBallRot>| calc_ball_rot.0),
                                            update_ball,
                                        )
                                            .chain(),
                                        (
                                            pre_update_car,
                                            (update_car, update_car_extra, update_car_wheels),
                                            update_camera,
                                        )
                                            .chain(),
                                        (update_pads_count, update_pad_colors).chain(),
                                        update_field,
                                    )
                                        .run_if(|updated: Res<PacketUpdated>| updated.0),
                                    (
                                        (
                                            (extrapolate_packet, update_ball_rotation),
                                            (update_ball, (update_car, update_camera).chain(), update_car_wheels),
                                        )
                                            .chain()
                                            .run_if(|ps: Res<PacketSmoothing>| matches!(*ps, PacketSmoothing::Extrapolate)),
                                        (
                                            interpolate_packets,
                                            (update_ball, (update_car, update_camera).chain(), update_car_wheels),
                                        )
                                            .chain()
                                            .run_if(|ps: Res<PacketSmoothing>| matches!(*ps, PacketSmoothing::Interpolate)),
                                    )
                                        .run_if(|updated: Res<PacketUpdated>| !updated.0),
                                ),
                                (
                                    listen,
                                    update_boost_meter,
                                    (dropshot_update_ball, update_tiles)
                                        .run_if(|game_mode: Res<GameMode>| *game_mode == GameMode::Dropshot),
                                ),
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
