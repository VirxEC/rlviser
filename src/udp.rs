use std::{cmp::Ordering, f32::consts::PI, fs, net::UdpSocket, time::Duration};

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
    gui::BallCam,
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
struct DirectorTimer(Timer);

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
        On::<Pointer<Over>>::target_insert(HighlightedEntity),
        On::<Pointer<Out>>::target_remove::<HighlightedEntity>(),
    ));
}

#[inline]
/// Use colors that are a bit darker if we don't have the full_load feature
fn get_color_from_team(team: Team) -> Color {
    match team {
        Team::Blue => {
            if cfg!(feature = "full_load") {
                Color::rgb(0.03, 0.09, 0.79)
            } else {
                Color::rgb(0.01, 0.03, 0.39)
            }
        }
        Team::Orange => {
            if cfg!(feature = "full_load") {
                Color::rgb(0.41, 0.21, 0.01)
            } else {
                Color::rgb(0.82, 0.42, 0.02)
            }
        }
    }
}

fn spawn_car(
    car_info: &CarInfo,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
) {
    let hitbox = car_info.config.hitbox_size.to_bevy();
    let base_color = get_color_from_team(car_info.team);

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

    let Some(mesh_info) = get_mesh_info(mesh_id, meshes) else {
        return;
    };

    let mesh_materials = if cfg!(feature = "full_load") {
        get_car_mesh_materials(mesh_id, materials, asset_server, base_color)
    } else {
        vec![materials.add(base_color.into()); mesh_info.len()]
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
            On::<Pointer<Over>>::target_insert(HighlightedEntity),
            On::<Pointer<Out>>::target_remove::<HighlightedEntity>(),
            On::<Pointer<Drag>>::send_event::<ChangeCarPos>(),
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

fn step_arena(
    socket: Res<UdpConnection>,
    mut game_state: ResMut<GameState>,
    mut exit: EventWriter<AppExit>,
    mut packet_updated: ResMut<PacketUpdated>,
) {
    let mut temp_buf = None;

    const PACKET_TYPE_BUFFER: [u8; 1] = [0];
    let mut packet_type = PACKET_TYPE_BUFFER;

    while socket.0.recv_from(&mut packet_type).is_ok() {
        let Some(packet_type) = UdpPacketTypes::new(packet_type[0]) else {
            return;
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
            return;
        }

        let mut full_buf = vec![0; GameState::get_num_bytes(&min_buf)];
        if socket.0.recv_from(&mut full_buf).is_err() {
            return;
        }

        temp_buf = Some(full_buf);
    }

    let Some(buf) = temp_buf else {
        packet_updated.0 = false;
        return;
    };

    packet_updated.0 = true;
    *game_state = GameState::from_bytes(&buf);
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

const MIN_DIST_FROM_BALL: f32 = 200.;
const MIN_DIST_FROM_BALL_SQ: f32 = MIN_DIST_FROM_BALL * MIN_DIST_FROM_BALL;

const MIN_CAMERA_BALLCAM_HEIGHT: f32 = 20.;

#[allow(clippy::too_many_arguments)]
fn update_car(
    time: Res<Time>,
    state: Res<GameState>,
    ballcam: Res<BallCam>,
    asset_server: Res<AssetServer>,
    car_entities: Query<(Entity, &Car)>,
    mut cars: Query<(&mut Transform, &Car)>,
    mut camera_query: Query<(&mut PrimaryCamera, &mut Transform), Without<Car>>,
    mut timer: ResMut<DirectorTimer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    match cars.iter().count().cmp(&state.cars.len()) {
        Ordering::Greater => {
            for (entity, car) in &car_entities {
                if !state.cars.iter().any(|car_info| car.0 == car_info.id) {
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
        Ordering::Less => {
            let all_current_cars = cars.iter().map(|(_, car)| car.0).collect::<Vec<_>>();
            let non_existant_cars = state
                .cars
                .iter()
                .filter(|car_info| !all_current_cars.iter().any(|&id| id == car_info.id));

            for car_info in non_existant_cars {
                spawn_car(car_info, &mut commands, &mut meshes, &mut materials, &asset_server);
            }
        }
        _ => {}
    }

    timer.0.tick(time.delta());

    let (mut primary_camera, mut camera_transform) = camera_query.single_mut();

    let car_id = match primary_camera.as_mut() {
        PrimaryCamera::TrackCar(id) => *id,
        PrimaryCamera::Director(id) => {
            if *id == 0 || timer.0.finished() {
                // get the car closest to the ball
                let mut min_dist = f32::MAX;
                for car in &state.cars {
                    let dist = car.state.pos.distance_squared(state.ball.pos);
                    if dist < min_dist {
                        *id = car.id;
                        min_dist = dist;
                    }
                }
            }

            *id
        }
        _ => 0,
    };

    for (mut car_transform, car) in cars.iter_mut() {
        let car_state = &state.cars.iter().find(|car_info| car.0 == car_info.id).unwrap().state;
        car_transform.translation = car_state.pos.to_bevy();
        car_transform.rotation = car_state.rot_mat.to_bevy();

        if car_id == car.id() {
            let camera_transform = camera_transform.as_mut();

            if ballcam.enabled
                && (!car_state.is_on_ground || car_state.pos.distance_squared(state.ball.pos) > MIN_DIST_FROM_BALL_SQ)
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
                let car_look = Vec3::new(car_state.vel.x, 0., car_state.vel.y)
                    .try_normalize()
                    .unwrap_or_else(|| car_transform.forward());
                camera_transform.translation = car_transform.translation - car_look * 280. + Vec3::Y * 110.;
                camera_transform.look_to(car_look, Vec3::Y);
                camera_transform.rotation *= Quat::from_rotation_x(-PI / 30.);
            }
        }
    }
}

fn update_pads(
    state: Res<GameState>,
    pads: Query<(Entity, &BoostPadI)>,
    query: Query<&Handle<StandardMaterial>, With<BoostPadI>>,
    pad_glows: Res<BoostPickupGlows>,
    large_boost_pad_loc_rots: Res<LargeBoostPadLocRots>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    if pads.iter().count() != state.pads.len() {
        // The number of pads shouldn't change often
        // There's also not an easy way to determine
        // if a previous pad a new pad are same pad
        // It is the easiest to despawn and respawn all pads
        for (entity, _) in pads.iter() {
            commands.entity(entity).despawn_recursive();
        }

        for pad in &state.pads {
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
                On::<Pointer<Over>>::target_insert(HighlightedEntity),
                On::<Pointer<Out>>::target_remove::<HighlightedEntity>(),
            ));
        }
    }

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

#[derive(Resource)]
struct PacketUpdated(bool);

pub struct RocketSimPlugin;

impl Plugin for RocketSimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameState::default())
            .insert_resource(DirectorTimer(Timer::new(Duration::from_secs(12), TimerMode::Repeating)))
            .insert_resource(PacketUpdated(false))
            .add_systems(
                Update,
                (
                    establish_connection.run_if(in_state(LoadState::Connect)),
                    (
                        step_arena,
                        (update_ball, update_car, update_pads).run_if(|updated: Res<PacketUpdated>| updated.0),
                        listen,
                    )
                        .chain()
                        .run_if(in_state(LoadState::None)),
                ),
            );
    }
}
