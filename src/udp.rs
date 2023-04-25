use std::{cmp::Ordering, f32::consts::PI, fs, net::UdpSocket};

use bevy::{
    math::{Mat3A, Vec3A},
    prelude::*,
};
use bevy_mod_picking::PickableBundle;

use crate::{
    assets::{get_material, get_mesh_info},
    bytes::{FromBytes, ToBytes},
    camera::EntityName,
    rocketsim::{CarInfo, GameState, Team},
    LoadState, ServerPort,
};

#[derive(Component)]
struct BoostPadI;

#[derive(Component)]
pub struct Ball;

#[derive(Component)]
pub struct Car(u32);

impl Car {
    pub fn id(&self) -> u32 {
        self.0
    }
}

#[derive(Resource)]
struct UdpConnection(UdpSocket);

fn establish_connection(port: Res<ServerPort>, mut commands: Commands, mut state: ResMut<NextState<LoadState>>) {
    let socket = UdpSocket::bind(("127.0.0.1", port.secondary_port)).unwrap();
    socket.connect(("127.0.0.1", port.primary_port)).unwrap();
    socket.send(&[1]).unwrap();
    socket.set_nonblocking(true).unwrap();
    commands.insert_resource(UdpConnection(socket));
    state.set(LoadState::None);
}

pub trait ToBevyVec {
    fn to_bevy(self) -> Vec3;
}

impl ToBevyVec for [f32; 3] {
    fn to_bevy(self) -> Vec3 {
        Vec3::new(self[0], self[2], self[1])
    }
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
        let mat = Mat3A::from_axis_angle(Vec3::Y, PI)
            * Mat3A::from_axis_angle(Vec3::X, PI / 2.)
            * self
            * Mat3A::from_cols(Vec3A::X, -Vec3A::Z, Vec3A::Y)
            * Mat3A::from_axis_angle(Vec3::Y, PI);
        Quat::from_mat3a(&mat)
    }
}

trait ToBevyQuat {
    fn to_bevy(self) -> Quat;
}

impl ToBevyQuat for Quat {
    fn to_bevy(self) -> Quat {
        // In RocketSim, the Z axis is up, but in Bevy, the Z and Y axis are swapped
        // We also need to rotate 90 degrees around the X axis and 180 degrees around the Y axis
        Quat::from_axis_angle(Vec3::Y, PI) * Quat::from_axis_angle(Vec3::X, PI / 2.) * self * Quat::from_mat3a(&Mat3A::from_cols(Vec3A::X, -Vec3A::Z, Vec3A::Y))
    }
}

const CAR_BODIES: [(&str, &str); 3] = [
    ("octane_body", "Body_Octane.SkeletalMesh3.Body_Octane_SK"),
    ("dominus_body", "Body_MuscleCar.SkeletalMesh3.Body_MuscleCar_SK"),
    ("plank_body", "Body_Darkcar.SkeletalMesh3.Body_Darkcar_SK"),
];

fn spawn_default_car(id: u32, base_color: Color, hitbox: Vec3, commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
    commands.spawn((
        Car(id),
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Box::new(hitbox.x, hitbox.y, hitbox.z))),
            material: materials.add(base_color.into()),
            ..Default::default()
        },
        PickableBundle::default(),
        EntityName::new("generic_body"),
    ));
}

fn spawn_car(car_info: &CarInfo, commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, asset_server: &AssetServer) {
    let hitbox = car_info.config.hitbox_size.to_bevy();
    let base_color = match car_info.team {
        Team::Blue => Color::rgb(0.03, 0.09, 0.79),
        Team::Orange => Color::rgb(0.82, 0.42, 0.02),
    };

    let (name, mesh_id) = CAR_BODIES[if (120f32..121.).contains(&hitbox.x) {
        // octane
        0
    } else if (130f32..131.).contains(&hitbox.x) {
        // dominus
        1
    } else if (131f32..132.).contains(&hitbox.x) {
        // plank
        2
    } else {
        spawn_default_car(car_info.id, base_color, hitbox, commands, meshes, materials);

        return;
    }];

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

    let Some(mesh_info) = get_mesh_info(mesh_id, meshes) else {
        return;
    };

    commands
        .spawn((
            Car(car_info.id),
            GlobalTransform::default(),
            Transform::default(),
            Visibility::default(),
            ComputedVisibility::default(),
            PickableBundle::default(),
            EntityName::new(name),
        ))
        .with_children(|parent| {
            mesh_info
                .into_iter()
                .zip(mesh_materials)
                .map(|(mesh, material)| PbrBundle {
                    mesh,
                    material,
                    ..Default::default()
                })
                .for_each(|bundle| {
                    parent.spawn(bundle);
                });
        });
}

#[allow(clippy::too_many_arguments)]
fn step_arena(
    socket: Res<UdpConnection>,
    cars: Query<(Entity, &Car)>,
    pads: Query<(Entity, &BoostPadI)>,
    asset_server: Res<AssetServer>,
    mut game_state: ResMut<GameState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut buf = None;
    loop {
        const INITIAL_BUFFER: [u8; GameState::MIN_NUM_BYTES] = [0; GameState::MIN_NUM_BYTES];
        let mut min_buf = INITIAL_BUFFER;
        if socket.0.peek_from(&mut min_buf).is_err() {
            break;
        }

        if game_state.tick_count > GameState::read_tick_count(&min_buf) {
            drop(socket.0.recv_from(&mut [0]));
            break;
        }

        let mut next_buf = vec![0; GameState::get_num_bytes(&min_buf)];
        if socket.0.recv_from(&mut next_buf).is_err() {
            break;
        }
        buf = Some(next_buf);
    }

    let Some(buf) = buf else {
        return;
    };

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
                PickableBundle::default(),
                EntityName::new("generic_boost_pad"),
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
                spawn_car(car_info, &mut commands, &mut meshes, &mut materials, &asset_server);
            }
        }
        _ => {}
    }
}

fn update_ball(state: Res<GameState>, mut ball: Query<(&mut Transform, &Children), With<Ball>>, mut point_light: Query<&mut PointLight>) {
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

    transform.rotation = state.ball_rot.to_bevy();
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
        game_state.ball.vel = Vec3A::new(50., 1500., 1.);
    }

    if changed {
        socket.0.send(&game_state.to_bytes()).unwrap();
    }
}
pub struct RocketSimPlugin;

impl Plugin for RocketSimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameState::default())
            .add_system(establish_connection.run_if(in_state(LoadState::Connect)))
            .add_system(step_arena.run_if(in_state(LoadState::None)))
            .add_systems((update_ball, update_car, update_pads).after(step_arena).before(listen))
            .add_system(update_ball)
            .add_system(update_car)
            .add_system(update_pads)
            .add_system(listen.run_if(in_state(LoadState::None)));
    }
}
