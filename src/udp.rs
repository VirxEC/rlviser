use std::{cmp::Ordering, f32::consts::PI, net::UdpSocket};

use bevy::{
    math::{Mat3A, Vec3A},
    prelude::*,
};

use crate::{
    bytes::{FromBytes, ToBytesVec},
    ServerPort,
};

#[derive(Component)]
struct BoostPad;

#[derive(Component)]
struct Ball;

#[derive(Component)]
struct Car(u32);

#[derive(Clone, Copy, Default, Debug)]
pub struct BallState {
    pub pos: Vec3,
    pub vel: Vec3,
    pub ang_vel: Vec3,
}

#[repr(u8)]
#[derive(Clone, Copy, Default, Debug)]
pub enum Team {
    #[default]
    Blue,
    Orange,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct CarState {
    pub pos: Vec3,
    pub rot_mat: Mat3A,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct CarConfig {
    pub hitbox_size: Vec3,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct CarInfo {
    pub id: u32,
    pub team: Team,
    pub state: CarState,
    pub config: CarConfig,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct BoostPadState {
    pub is_active: bool,
    pub cooldown: f32,
    pub cur_locked_car_id: u32,
    pub prev_locked_car_id: u32,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct BoostPadInfo {
    pub is_big: bool,
    pub position: Vec3,
    pub state: BoostPadState,
}

#[derive(Resource, Default, Debug)]
pub struct GameState {
    pub tick_count: u64,
    pub ball: BallState,
    pub pads: Vec<BoostPadInfo>,
    pub cars: Vec<CarInfo>,
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

trait ViserVec {
    fn to_bevy(self) -> Vec3;
    fn to_rocket(self) -> Vec3;
}

impl ViserVec for Vec3 {
    fn to_bevy(self) -> Self {
        Self::new(self.x, self.z, self.y)
    }

    fn to_rocket(self) -> Vec3 {
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

fn step_arena(
    socket: Res<UdpConnection>,
    cars: Query<(Entity, &Car)>,
    pads: Query<(Entity, &BoostPad)>,
    mut game_state: ResMut<GameState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    const INITIAL_BUFFER: [u8; 16] = [0; 16];
    let mut buf = INITIAL_BUFFER;
    if socket.0.peek_from(&mut buf).is_err() {
        return;
    }

    let tick_count = u64::from_bytes(&buf[..u64::NUM_BYTES]);
    let num_pads = u32::from_bytes(&buf[u64::NUM_BYTES..u64::NUM_BYTES + u32::NUM_BYTES]) as usize;
    let num_cars = u32::from_bytes(&buf[u64::NUM_BYTES + u32::NUM_BYTES..]) as usize;

    if game_state.tick_count > tick_count {
        drop(socket.0.recv_from(&mut [0]));
    }

    let mut buf = vec![0; INITIAL_BUFFER.len() + BallState::NUM_BYTES + num_pads * BoostPadInfo::NUM_BYTES + num_cars * CarInfo::NUM_BYTES];
    if socket.0.recv_from(&mut buf).is_err() {
        return;
    }

    game_state.ball = BallState::from_bytes(&buf[INITIAL_BUFFER.len()..INITIAL_BUFFER.len() + BallState::NUM_BYTES]);

    if game_state.pads.len() != num_pads {
        game_state.pads = vec![BoostPadInfo::default(); num_pads];
    }

    for (i, pad) in game_state.pads.iter_mut().enumerate() {
        let start_byte = INITIAL_BUFFER.len() + BallState::NUM_BYTES + i * BoostPadInfo::NUM_BYTES;
        *pad = BoostPadInfo::from_bytes(&buf[start_byte..(start_byte + BoostPadInfo::NUM_BYTES)]);
    }

    if pads.iter().count() != num_pads {
        // The number of pads shouldn't change often
        // There's also not an easy way to determine
        // if a previous pad a new pad are same pad
        // It is the easiest to despawn and respawn all pads
        for (entity, _) in pads.iter() {
            commands.entity(entity).despawn_recursive();
        }

        for pad in &game_state.pads {
            // nice yellow color for active pads
            let color = Color::rgba(0.9, 0.9, 0.1, 0.6);

            let shape = if pad.is_big {
                shape::Cylinder {
                    radius: 208.,
                    height: 168.,
                    ..default()
                }
            } else {
                shape::Cylinder {
                    radius: 144.,
                    height: 165.,
                    ..default()
                }
            };

            commands.spawn((
                BoostPad,
                PbrBundle {
                    mesh: meshes.add(Mesh::from(shape)),
                    material: materials.add(StandardMaterial::from(color)),
                    transform: Transform::from_translation(pad.position.to_bevy() + Vec3::Y),
                    ..default()
                },
            ));
        }
    }

    if game_state.cars.len() != num_cars {
        game_state.cars.resize_with(num_cars, Default::default);
    }

    for (i, car) in game_state.cars.iter_mut().enumerate() {
        let start_byte = INITIAL_BUFFER.len() + BallState::NUM_BYTES + num_pads * BoostPadInfo::NUM_BYTES + i * CarInfo::NUM_BYTES;
        *car = CarInfo::from_bytes(&buf[start_byte..(start_byte + CarInfo::NUM_BYTES)]);
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

fn update_pads(state: Res<GameState>, query: Query<&Handle<StandardMaterial>, With<BoostPad>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    for (pad, handle) in state.pads.iter().zip(query.iter()) {
        let material = materials.get_mut(handle).unwrap();
        material.base_color = if pad.state.is_active {
            Color::rgba(0.9, 0.9, 0.1, 0.6)
        } else {
            // make inactive pads grey and more transparent
            Color::rgba(0.5, 0.5, 0.5, 0.3)
        };
    }
}

fn listen(socket: Res<UdpConnection>, key: Res<Input<KeyCode>>, mut game_state: ResMut<GameState>) {
    let mut changed = false;
    if key.just_pressed(KeyCode::R) {
        changed = true;

        game_state.ball.pos = Vec3::new(0., -2000., 1500.);
        game_state.ball.vel = Vec3::new(0., 1500., 1.);
    }

    if changed {
        socket.0.send(&game_state.to_bytes()).unwrap();
    }
}
pub struct RocketSimPlugin;

impl Plugin for RocketSimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameState::default())
            .add_startup_system(establish_connection)
            .add_startup_system(setup_arena)
            .add_system(step_arena)
            .add_systems((update_ball, update_car, update_pads).after(step_arena).before(listen))
            .add_system(update_ball.run_if(|state: Res<GameState>| state.is_changed()))
            .add_system(update_car.run_if(|state: Res<GameState>| state.is_changed()))
            .add_system(update_pads.run_if(|state: Res<GameState>| state.is_changed()))
            .add_system(listen);
    }
}
