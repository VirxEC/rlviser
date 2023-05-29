use std::{cmp::Ordering, f32::consts::PI, fs, net::UdpSocket};

use bevy::{
    app::AppExit,
    math::{Mat3A, Vec3A, Vec3Swizzles},
    prelude::*,
};
use bevy_mod_picking::prelude::*;

use crate::{
    assets::{get_material, get_mesh_info, BoostPickupGlows},
    bytes::{FromBytes, ToBytes},
    camera::{EntityName, HighlightedEntity, PrimaryCamera},
    mesh::{ChangeCarPos, LargeBoostPadLocRots},
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
    #[inline]
    pub fn id(&self) -> u32 {
        self.0
    }
}

#[derive(Resource)]
pub struct UdpConnection(pub UdpSocket);

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

trait ToBevyMat {
    fn to_bevy(self) -> Quat;
}

impl ToBevyMat for Mat3A {
    #[inline]
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
    #[inline]
    fn to_bevy(self) -> Quat {
        // In RocketSim, the Z axis is up, but in Bevy, the Z and Y axis are swapped
        // We also need to rotate 90 degrees around the X axis and 180 degrees around the Y axis
        Quat::from_axis_angle(Vec3::Y, PI)
            * Quat::from_axis_angle(Vec3::X, PI / 2.)
            * self
            * Quat::from_mat3a(&Mat3A::from_cols(Vec3A::X, -Vec3A::Z, Vec3A::Y))
    }
}

const CAR_BODIES: [(&str, &str); 6] = [
    ("octane_body", "Body_Octane.SkeletalMesh3.Body_Octane_SK"),
    ("dominus_body", "Body_MuscleCar.SkeletalMesh3.Body_MuscleCar_SK"),
    ("plank_body", "Body_Darkcar.SkeletalMesh3.Body_Darkcar_SK"),
    ("breakout_body", "Body_Force.SkeletalMesh3.Body_Force_PremiumSkin_SK"),
    ("hybrid_body", "Body_Venom.SkeletalMesh3.Body_Venom_PremiumSkin_SK"),
    ("merc_body", "Body_Vanquish.SkeletalMesh3.Body_Merc_PremiumSkin_SK"),
];

fn spawn_default_car(
    id: u32,
    base_color: Color,
    hitbox: Vec3,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    commands.spawn((
        Car(id),
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Box::new(hitbox.x, hitbox.y, hitbox.z))),
            material: materials.add(base_color.into()),
            ..Default::default()
        },
        EntityName::new("generic_body"),
        RaycastPickTarget::default(),
        OnPointer::<Over>::target_insert(HighlightedEntity),
        OnPointer::<Out>::target_remove::<HighlightedEntity>(),
    ));
}

fn spawn_car(
    car_info: &CarInfo,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
) {
    let hitbox = car_info.config.hitbox_size.to_bevy();

    #[cfg(feature = "full_load")]
    let base_color = match car_info.team {
        Team::Blue => Color::rgb(0.03, 0.09, 0.79),
        Team::Orange => Color::rgb(0.82, 0.42, 0.02),
    };

    #[cfg(not(feature = "full_load"))]
    // use colors that are a bit darker
    let base_color = match car_info.team {
        Team::Blue => Color::rgb(0.01, 0.03, 0.39),
        Team::Orange => Color::rgb(0.41, 0.21, 0.01),
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
            PbrBundle {
                mesh: meshes.add(shape::Box::new(hitbox.x * 2., hitbox.y * 2., hitbox.z * 2.).into()),
                material: materials.add(Color::NONE.into()),
                ..Default::default()
            },
            EntityName::new(name),
            RaycastPickTarget::default(),
            OnPointer::<Over>::target_insert(HighlightedEntity),
            OnPointer::<Out>::target_remove::<HighlightedEntity>(),
            OnPointer::<Drag>::send_event::<ChangeCarPos>(),
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

#[repr(u8)]
#[derive(PartialEq, Eq)]
enum UdpPacketTypes {
    Quit,
    GameState,
}

impl UdpPacketTypes {
    fn new(byte: u8) -> Option<Self> {
        if byte == 0 {
            Some(Self::Quit)
        } else if byte == 1 {
            Some(Self::GameState)
        } else {
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn step_arena(
    socket: Res<UdpConnection>,
    cars: Query<(Entity, &Car)>,
    pads: Query<(Entity, &BoostPadI)>,
    asset_server: Res<AssetServer>,
    pad_glows: Res<BoostPickupGlows>,
    large_boost_pad_loc_rots: Res<LargeBoostPadLocRots>,
    mut game_state: ResMut<GameState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut exit: EventWriter<AppExit>,
) {
    let buf = loop {
        const PACKET_TYPE_BUFFER: [u8; 1] = [0];
        let mut packet_type = PACKET_TYPE_BUFFER;
        if socket.0.recv_from(&mut packet_type).is_err() {
            return;
        }

        let Some(packet_type) = UdpPacketTypes::new(packet_type[0]) else {
            continue;
        };

        if packet_type != UdpPacketTypes::GameState {
            // quit bevy app
            exit.send(AppExit);
            return;
        }

        const INITIAL_BUFFER: [u8; GameState::MIN_NUM_BYTES] = [0; GameState::MIN_NUM_BYTES];
        let mut min_buf = INITIAL_BUFFER;
        // wait until we receive the packet
        // it should arrive VERY quickly, so a loop with no delay is fine
        // if it doesn't, then there are other problems lol
        while socket.0.peek_from(&mut min_buf).is_err() {}

        if game_state.tick_count > GameState::read_tick_count(&min_buf) {
            drop(socket.0.recv_from(&mut [0]));
            continue;
        }

        let mut buf = vec![0; GameState::get_num_bytes(&min_buf)];
        if socket.0.recv_from(&mut buf).is_err() {
            continue;
        }

        break buf;
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
            let mut transform = Transform::from_translation(pad.position.to_bevy() - Vec3::Y * 70.);

            let mesh = if pad.is_big {
                let rotation = large_boost_pad_loc_rots
                    .locs
                    .iter()
                    .enumerate()
                    .find(|(_, loc)| loc.distance_squared(pad.position.xy()) < 25.)
                    .map(|(i, _)| large_boost_pad_loc_rots.rots[i]);
                transform.rotate_y(rotation.unwrap_or_default().to_radians());

                pad_glows.large.clone()
            } else {
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

                pad_glows.small.clone()
            };

            commands.spawn((
                BoostPadI,
                PbrBundle {
                    mesh,
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
                EntityName::new("generic_boost_pad"),
                RaycastPickTarget::default(),
                OnPointer::<Over>::target_insert(HighlightedEntity),
                OnPointer::<Out>::target_remove::<HighlightedEntity>(),
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
            let non_existant_cars = game_state
                .cars
                .iter()
                .filter(|car_info| !all_current_cars.iter().any(|&id| id == car_info.id));

            for car_info in non_existant_cars {
                spawn_car(car_info, &mut commands, &mut meshes, &mut materials, &asset_server);
            }
        }
        _ => {}
    }
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

    transform.rotation = state.ball_rot.to_bevy();
}

fn update_car(
    state: Res<GameState>,
    mut cars: Query<(&mut Transform, &Car)>,
    mut camera_query: Query<(&PrimaryCamera, &mut Transform), Without<Car>>,
) {
    let mut camera_info = if let Ok((PrimaryCamera::TrackCar(car_id), camera_transform)) = camera_query.get_single_mut() {
        Some((*car_id, camera_transform))
    } else {
        None
    };

    for (mut car_transform, car) in cars.iter_mut() {
        let car_state = &state.cars.iter().find(|car_info| car.0 == car_info.id).unwrap().state;
        car_transform.translation = car_state.pos.to_bevy();
        car_transform.rotation = car_state.rot_mat.to_bevy();

        if let Some((car_id, ref mut camera_transform)) = camera_info {
            if car_id == car.id() {
                let camera_transform = camera_transform.as_mut();
                camera_transform.translation =
                    car_transform.translation - car_transform.right() * 300. + car_transform.up() * 150.;
                camera_transform.look_to(car_transform.forward(), car_transform.up());
                camera_transform.rotation *= Quat::from_rotation_y(-PI / 2.) * Quat::from_rotation_x(-PI / 16.);
            }
        }
    }
}

fn update_pads(
    state: Res<GameState>,
    query: Query<&Handle<StandardMaterial>, With<BoostPadI>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (pad, handle) in state.pads.iter().zip(query.iter()) {
        let material = materials.get_mut(handle).unwrap();
        material.base_color = if pad.state.is_active {
            Color::rgba(0.9, 0.9, 0.1, 0.6)
        } else {
            // make the glow on inactive pads dissapear
            Color::NONE
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
