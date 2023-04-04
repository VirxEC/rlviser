use std::{cmp::Ordering, f32::consts::PI, net::UdpSocket};

use bevy::{
    math::{Mat3A, Vec3A},
    prelude::*,
};

use crate::ServerPort;

#[derive(Component)]
struct Ball;

#[derive(Component)]
struct Car(u32);

#[derive(Default, Debug)]
struct BallState {
    pos: Vec3,
}

#[repr(u8)]
#[derive(Default, Debug)]
enum Team {
    #[default]
    Blue,
    Orange,
}

#[derive(Default, Debug)]
struct CarState {
    pos: Vec3,
    rot_mat: Mat3A,
}

#[derive(Default, Debug)]
struct CarConfig {
    hitbox_size: Vec3,
}

#[derive(Default, Debug)]
struct CarInfo {
    id: u32,
    team: Team,
    state: CarState,
    config: CarConfig,
}

#[derive(Resource, Default, Debug)]
struct GameState {
    tick_count: u64,
    ball: BallState,
    cars: Vec<CarInfo>,
}

#[derive(Resource)]
struct UdpConnection(UdpSocket);

fn establish_connection(port: Res<ServerPort>, mut commands: Commands) {
    let socket = UdpSocket::bind(("127.0.0.1", port.secondary_port)).unwrap();
    socket.connect(("127.0.0.1", port.primary_port)).unwrap();
    socket.set_nonblocking(true).unwrap();
    socket.send(&[1]).unwrap();
    commands.insert_resource(UdpConnection(socket));
}

fn setup_arena(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut ball_material = StandardMaterial::from(Color::rgb(0.3, 0.3, 0.3));
    ball_material.perceptual_roughness = 0.8;

    commands.spawn((
        Ball,
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::UVSphere { radius: 91.25, ..default() })),
            material: materials.add(ball_material),
            ..default()
        },
    ));
}

trait ToBevyVec {
    fn to_bevy(self) -> Vec3;
}

impl ToBevyVec for Vec3 {
    fn to_bevy(self) -> Self {
        Self::new(self.x, self.z, self.y)
    }
}

trait ToBevyMat {
    fn to_bevy(self) -> Quat;
}

impl ToBevyMat for Mat3A {
    fn to_bevy(self) -> Quat {
        // In RocketSim, the Z axis is up, but in Bevy, the Z and Y axis are swapped
        // We also need to rotate 90 degrees around the X axis and 180 degrees around the Y axis
        let mat = Mat3A::from_axis_angle(Vec3::Y, PI) * Mat3A::from_axis_angle(Vec3::X, PI / 2.) * self * Mat3A::from_cols(Vec3A::X, -Vec3A::Z, Vec3A::Y);
        Quat::from_mat3a(&mat)
    }
}

trait FromBytes {
    fn num_bytes() -> usize;
    fn from_bytes(bytes: &[u8]) -> Self;
}

impl FromBytes for f32 {
    #[inline]
    fn num_bytes() -> usize {
        4
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}

impl FromBytes for u32 {
    #[inline]
    fn num_bytes() -> usize {
        4
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}

impl FromBytes for u64 {
    #[inline]
    fn num_bytes() -> usize {
        8
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        u64::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]])
    }
}

impl FromBytes for Vec3 {
    #[inline]
    fn num_bytes() -> usize {
        f32::num_bytes() * 3
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Vec3::new(f32::from_bytes(&bytes[..4]), f32::from_bytes(&bytes[4..8]), f32::from_bytes(&bytes[8..12]))
    }
}

impl FromBytes for Vec3A {
    #[inline]
    fn num_bytes() -> usize {
        f32::num_bytes() * 3
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Vec3A::new(f32::from_bytes(&bytes[..4]), f32::from_bytes(&bytes[4..8]), f32::from_bytes(&bytes[8..12]))
    }
}

impl FromBytes for Mat3A {
    #[inline]
    fn num_bytes() -> usize {
        Vec3A::num_bytes() * 3
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Mat3A::from_cols(Vec3A::from_bytes(&bytes[..12]), Vec3A::from_bytes(&bytes[12..24]), Vec3A::from_bytes(&bytes[24..36]))
    }
}

impl FromBytes for BallState {
    #[inline]
    fn num_bytes() -> usize {
        Vec3::num_bytes()
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            pos: Vec3::from_bytes(&bytes[..12]),
        }
    }
}

impl FromBytes for Team {
    #[inline]
    fn num_bytes() -> usize {
        1
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        match bytes[0] {
            0 => Team::Blue,
            1 => Team::Orange,
            _ => unreachable!(),
        }
    }
}

impl FromBytes for CarState {
    #[inline]
    fn num_bytes() -> usize {
        Vec3::num_bytes() + Mat3A::num_bytes()
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            pos: Vec3::from_bytes(&bytes[..12]),
            rot_mat: Mat3A::from_bytes(&bytes[12..Self::num_bytes()]),
        }
    }
}

impl FromBytes for CarConfig {
    #[inline]
    fn num_bytes() -> usize {
        Vec3::num_bytes()
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            hitbox_size: Vec3::from_bytes(&bytes[..12]),
        }
    }
}

impl FromBytes for CarInfo {
    #[inline]
    fn num_bytes() -> usize {
        u32::num_bytes() + Team::num_bytes() + CarState::num_bytes() + CarConfig::num_bytes()
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            id: u32::from_bytes(&bytes[..4]),
            team: Team::from_bytes(&bytes[4..5]),
            state: CarState::from_bytes(&bytes[5..(5 + CarState::num_bytes())]),
            config: CarConfig::from_bytes(&bytes[(5 + CarState::num_bytes())..Self::num_bytes()]),
        }
    }
}

fn step_arena(
    socket: Res<UdpConnection>,
    cars: Query<(Entity, &Car)>,
    mut game_state: ResMut<GameState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    const INITIAL_BUFFER: [u8; 12] = [0; 12];
    let mut buf = INITIAL_BUFFER;
    if socket.0.peek_from(&mut buf).is_err() {
        return;
    }

    let tick_count = u64::from_bytes(&buf[..8]);
    let num_cars = u32::from_bytes(&buf[8..12]) as usize;

    if game_state.tick_count > tick_count {
        drop(socket.0.recv_from(&mut [0]));
    }

    if game_state.cars.len() != num_cars {
        game_state.cars.resize_with(num_cars, Default::default);
    }

    if num_cars == 0 {
        return;
    }

    let mut buf = vec![0; INITIAL_BUFFER.len() + BallState::num_bytes() + num_cars * CarInfo::num_bytes()];
    if socket.0.recv_from(&mut buf).is_err() {
        return;
    }

    game_state.ball = BallState::from_bytes(&buf[INITIAL_BUFFER.len()..INITIAL_BUFFER.len() + BallState::num_bytes()]);

    for (i, car) in game_state.cars.iter_mut().enumerate() {
        let start_byte = INITIAL_BUFFER.len() + BallState::num_bytes() + i * CarInfo::num_bytes();
        *car = CarInfo::from_bytes(&buf[start_byte..(start_byte + CarInfo::num_bytes())]);
    }

    match cars.iter().count().cmp(&game_state.cars.len()) {
        Ordering::Greater => {
            for (entity, car) in cars.iter() {
                if !game_state.cars.iter().any(|car_info| car.0 == car_info.id) {
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
        Ordering::Less => {
            let all_current_cars = cars.iter().map(|(_, car)| car.0).collect::<Vec<_>>();
            let non_existant_cars = game_state.cars.iter().filter(|car_info| !all_current_cars.iter().any(|&id| id == car_info.id));

            for car_info in non_existant_cars {
                let hitbox = car_info.config.hitbox_size.to_bevy();
                let color = match car_info.team {
                    Team::Blue => Color::rgb(0.03, 0.09, 0.79),
                    Team::Orange => Color::rgb(0.82, 0.42, 0.02),
                };

                commands.spawn((
                    Car(car_info.id),
                    PbrBundle {
                        mesh: meshes.add(Mesh::from(shape::Box::new(hitbox.x, hitbox.y, hitbox.z))),
                        material: materials.add(StandardMaterial::from(color)),
                        transform: Transform::from_translation(car_info.state.pos.to_bevy()),
                        ..default()
                    },
                ));
            }
        }
        _ => {}
    }
}

fn update_ball(state: Res<GameState>, mut ball: Query<(&mut Transform, &Handle<StandardMaterial>), With<Ball>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let (mut transform, standard_material) = ball.single_mut();
    let new_pos = state.ball.pos.to_bevy();
    transform.translation = new_pos;

    let material = materials.get_mut(standard_material).unwrap();

    let amount = (transform.translation.z.abs() / 3500.).min(0.55);
    material.base_color = if new_pos.z > 0. {
        Color::rgb(amount.max(0.3), (amount * (2. / 3.)).max(0.3), 0.3)
    } else {
        Color::rgb(0.3, 0.3, amount.max(0.3))
    };
}

fn update_car(state: Res<GameState>, mut cars: Query<(&mut Transform, &Car)>) {
    for (mut transform, car) in cars.iter_mut() {
        let car_state = &state.cars.iter().find(|car_info| car.0 == car_info.id).unwrap().state;
        transform.translation = car_state.pos.to_bevy();
        transform.rotation = car_state.rot_mat.to_bevy();
    }
}

pub struct RocketSimPlugin;

impl Plugin for RocketSimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameState::default())
            .add_startup_system(establish_connection)
            .add_startup_system(setup_arena)
            .add_system(step_arena)
            .add_systems((update_ball, update_car).after(step_arena))
            .add_system(update_ball.run_if(|state: Res<GameState>| state.is_changed()))
            .add_system(update_car.run_if(|state: Res<GameState>| state.is_changed()));
    }
}
