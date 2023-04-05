use std::{cmp::Ordering, f32::consts::PI, net::UdpSocket};

use bevy::{
    math::{Mat3A, Vec3A},
    prelude::*,
};

use crate::{
    bytes::{FromBytes, ToBytes},
    rocketsim::{GameState, Team},
    ServerPort,
};

#[derive(Component)]
struct BoostPadI;

#[derive(Component)]
struct Ball;

#[derive(Component)]
struct Car(u32);

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
    let initial_ball_color = Color::rgb(0.5, 0.5, 0.5);
    let mut ball_material = StandardMaterial::from(initial_ball_color);
    ball_material.perceptual_roughness = 0.8;

    // make a glowing ball
    commands
        .spawn((
            Ball,
            PbrBundle {
                mesh: meshes.add(Mesh::from(shape::UVSphere { radius: 91.25, ..default() })),
                material: materials.add(ball_material),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn(PointLightBundle {
                point_light: PointLight {
                    color: initial_ball_color,
                    intensity: 2_000_000.,
                    range: 1000.,
                    ..default()
                },
                ..default()
            });
        });
}

trait ToBevyVec {
    fn to_bevy(self) -> Vec3;
}

impl ToBevyVec for Vec3A {
    fn to_bevy(self) -> Vec3 {
        Vec3::new(self.x, self.z, self.y)
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
    pads: Query<(Entity, &BoostPadI)>,
    mut game_state: ResMut<GameState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    const INITIAL_BUFFER: [u8; GameState::MIN_NUM_BYTES] = [0; GameState::MIN_NUM_BYTES];
    let mut min_buf = INITIAL_BUFFER;
    if socket.0.peek_from(&mut min_buf).is_err() {
        return;
    }

    if game_state.tick_count > GameState::read_tick_count(&min_buf) {
        drop(socket.0.recv_from(&mut [0]));
    }

    let mut buf = vec![0; GameState::get_num_bytes(&min_buf)];
    if socket.0.recv_from(&mut buf).is_err() {
        return;
    }

    *game_state = GameState::from_bytes(&buf);

    if pads.iter().count() != game_state.pads.len() {
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
                BoostPadI,
                PbrBundle {
                    mesh: meshes.add(Mesh::from(shape)),
                    material: materials.add(StandardMaterial::from(color)),
                    transform: Transform::from_translation(pad.position.to_bevy() + Vec3::Y),
                    ..default()
                },
            ));
        }
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

fn update_ball(
    state: Res<GameState>,
    mut ball: Query<(&mut Transform, &Handle<StandardMaterial>, &Children), With<Ball>>,
    mut point_light: Query<&mut PointLight>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (mut transform, standard_material, children) = ball.single_mut();
    let new_pos = state.ball.pos.to_bevy();
    transform.translation = new_pos;

    let material = materials.get_mut(standard_material).unwrap();

    let amount = ((transform.translation.z.abs() + 1500.) / 3500.).min(0.95);
    material.base_color = if new_pos.z > 0. {
        Color::rgb(amount.max(0.5), (amount * (2. / 3.)).max(0.5), 0.5)
    } else {
        Color::rgb(0.5, 0.5, amount.max(0.5))
    };

    let mut point_light = point_light.get_mut(children.first().copied().unwrap()).unwrap();

    let amount = (transform.translation.z.abs() + 1500.) / 3500.;
    point_light.color = if new_pos.z > 0. {
        Color::rgb(amount.max(0.5), (amount * (2. / 3.)).max(0.5), 0.5)
    } else {
        Color::rgb(0.5, 0.5, amount.max(0.5))
    };
}

fn update_car(state: Res<GameState>, mut cars: Query<(&mut Transform, &Car)>) {
    for (mut transform, car) in cars.iter_mut() {
        let car_state = &state.cars.iter().find(|car_info| car.0 == car_info.id).unwrap().state;
        transform.translation = car_state.pos.to_bevy();
        transform.rotation = car_state.rot_mat.to_bevy();
    }
}

fn update_pads(state: Res<GameState>, query: Query<&Handle<StandardMaterial>, With<BoostPadI>>, mut materials: ResMut<Assets<StandardMaterial>>) {
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

        game_state.ball.pos = Vec3A::new(0., -2000., 1500.);
        game_state.ball.vel = Vec3A::new(0., 1500., 1.);
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
