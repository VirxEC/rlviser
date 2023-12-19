use crate::{
    assets::{get_material, get_mesh_info, BoostPickupGlows},
    bytes::{FromBytes, ToBytes},
    camera::{BoostAmount, HighlightedEntity, PrimaryCamera, TimeDisplay, BOOST_INDICATOR_FONT_SIZE, BOOST_INDICATOR_POS},
    gui::{BallCam, ShowTime, UiScale},
    mesh::{ChangeCarPos, LargeBoostPadLocRots},
    rocketsim::{CarInfo, GameMode, GameState, Team},
    LoadState, ServerPort,
};
use bevy::{
    app::AppExit,
    math::{Mat3A, Vec3A, Vec3Swizzles},
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
    window::PrimaryWindow,
};
use bevy_mod_picking::{backends::raycast::RaycastPickable, prelude::*};
use bevy_vector_shapes::prelude::*;
use std::{cmp::Ordering, f32::consts::PI, fs, net::UdpSocket, time::Duration};

#[cfg(debug_assertions)]
use crate::camera::EntityName;

#[derive(Component)]
struct BoostPadI;

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
pub struct Connection(pub UdpSocket);

fn establish_connection(port: Res<ServerPort>, mut commands: Commands, mut state: ResMut<NextState<LoadState>>) {
    let socket = UdpSocket::bind(("127.0.0.1", port.secondary_port)).unwrap();
    socket.connect(("127.0.0.1", port.primary_port)).unwrap();
    socket.send(&[1]).unwrap();
    socket.set_nonblocking(true).unwrap();
    commands.insert_resource(Connection(socket));
    state.set(LoadState::FieldExtra);
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

#[inline]
/// Use colors that are a bit darker if we don't have the `full_load` feature
const fn get_color_from_team(team: Team) -> Color {
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

#[derive(Component)]
pub struct CarBoost;

fn spawn_car(
    car_info: &CarInfo,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
) {
    let hitbox = car_info.config.hitbox_size.to_bevy();
    let hitbox_offset = car_info.config.hitbox_pos_offset.to_bevy();
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
                ..default()
            },
            #[cfg(debug_assertions)]
            EntityName::from(name),
            RaycastPickable,
            On::<Pointer<Over>>::target_insert(HighlightedEntity),
            On::<Pointer<Out>>::target_remove::<HighlightedEntity>(),
            On::<Pointer<Drag>>::send_event::<ChangeCarPos>(),
        ))
        .with_children(|parent| {
            const CAR_BOOST_LENGTH: f32 = 50.;
            mesh_info
                .into_iter()
                .zip(mesh_materials)
                .map(|(mesh, material)| PbrBundle {
                    mesh,
                    material,
                    transform: Transform::from_translation(hitbox_offset),
                    ..default()
                })
                .for_each(|bundle| {
                    parent.spawn(bundle);
                });

            parent.spawn((
                MaterialMeshBundle {
                    mesh: meshes.add(Mesh::from(shape::Cylinder {
                        height: CAR_BOOST_LENGTH,
                        radius: 10.,
                        resolution: 16,
                        ..default()
                    })),
                    material: materials.add(StandardMaterial {
                        base_color: Color::Rgba {
                            red: 1.,
                            green: 1.,
                            blue: 0.,
                            alpha: 0.,
                        },
                        alpha_mode: AlphaMode::Add,
                        cull_mode: None,
                        ..default()
                    }),
                    transform: Transform {
                        translation: Vec3::new((hitbox.x + CAR_BOOST_LENGTH) / -2., hitbox.y / 2., 0.) + hitbox_offset,
                        rotation: Quat::from_rotation_z(PI / 2.),
                        ..default()
                    },
                    ..default()
                },
                CarBoost,
            ));
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
enum UdpPacketTypes {
    Quit,
    GameState,
}

impl UdpPacketTypes {
    const fn new(byte: u8) -> Option<Self> {
        if byte == 0 {
            Some(Self::Quit)
        } else if byte == 1 {
            Some(Self::GameState)
        } else {
            None
        }
    }
}

const PACKET_TYPE_BUFFER: [u8; 1] = [0];
static mut INITIAL_BUFFER: [u8; GameState::MIN_NUM_BYTES] = [0; GameState::MIN_NUM_BYTES];

fn step_arena(
    socket: Res<Connection>,
    mut game_state: ResMut<GameState>,
    mut exit: EventWriter<AppExit>,
    mut packet_updated: ResMut<PacketUpdated>,
) {
    let mut packet_type = PACKET_TYPE_BUFFER;

    packet_updated.0 = false;
    let mut buf = Vec::new();

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
                // wait until we receive the packet
                // it should arrive VERY quickly, so a loop with no delay is fine
                // if it doesn't, then there are other problems lol
                // UPDATE: Windows throws a specific error that we need to look for
                // despite the fact that it actually worked

                #[cfg(windows)]
                {
                    while let Err(e) = socket.0.peek_from(unsafe { &mut INITIAL_BUFFER }) {
                        if let Some(code) = e.raw_os_error() {
                            if code == 10040 {
                                break;
                            }
                        }
                    }
                }

                #[cfg(not(windows))]
                {
                    while socket.0.peek_from(unsafe { &mut INITIAL_BUFFER }).is_err() {}
                }

                let new_tick_count = GameState::read_tick_count(unsafe { &INITIAL_BUFFER });
                if new_tick_count > 1 && game_state.tick_count > new_tick_count {
                    drop(socket.0.recv_from(&mut [0]));
                    return;
                }

                buf.resize(GameState::get_num_bytes(unsafe { &INITIAL_BUFFER }), 0);
                if socket.0.recv_from(&mut buf).is_err() {
                    return;
                }
            }
        }
    }

    if buf.is_empty() {
        return;
    }

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

    transform.rotation = state.ball.rot_mat.to_bevy();
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
    mut cars: Query<(&mut Transform, &Car, &Children)>,
    mut car_boosts: Query<&Handle<StandardMaterial>, With<CarBoost>>,
    mut camera_query: Query<(&mut PrimaryCamera, &mut Transform), Without<Car>>,
    mut timer: ResMut<DirectorTimer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last_boost_states: Local<Vec<u32>>,
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
            let all_current_cars = cars.iter().map(|(_, car, _)| car.0).collect::<Vec<_>>();
            let non_existant_cars = state
                .cars
                .iter()
                .filter(|car_info| !all_current_cars.iter().any(|&id| id == car_info.id));

            for car_info in non_existant_cars {
                spawn_car(car_info, &mut commands, &mut meshes, &mut materials, &asset_server);
            }
        }
        Ordering::Equal => {}
    }

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

    for (mut car_transform, car, children) in &mut cars {
        let Some(target_car) = state.cars.iter().find(|car_info| car.0 == car_info.id) else {
            continue;
        };

        car_transform.translation = target_car.state.pos.to_bevy();
        car_transform.rotation = target_car.state.rot_mat.to_bevy();

        let is_boosting = target_car.state.last_controls.boost && target_car.state.boost > f32::EPSILON;
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
    if pads.iter().count() != state.pads.len() && !large_boost_pad_loc_rots.rots.is_empty() {
        // The number of pads shouldn't change often
        // There's also not an easy way to determine
        // if a previous pad a new pad are same pad
        // It is the easiest to despawn and respawn all pads
        for (entity, _) in pads.iter() {
            commands.entity(entity).despawn_recursive();
        }

        for pad in &*state.pads {
            let mut transform = Transform::from_translation(pad.position.to_bevy() - Vec3::Y * 70.);

            let mesh = if pad.is_big {
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

                pad_glows.large.clone()
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
                #[cfg(debug_assertions)]
                EntityName::from("generic_boost_pad"),
                RaycastPickable,
                On::<Pointer<Over>>::target_insert(HighlightedEntity),
                On::<Pointer<Out>>::target_remove::<HighlightedEntity>(),
                NotShadowCaster,
                NotShadowReceiver,
            ));
        }
    }

    for (pad, handle) in state.pads.iter().zip(query.iter()) {
        materials.get_mut(handle).unwrap().base_color.set_a(if pad.state.is_active {
            0.6
        } else {
            // make the glow on inactive pads dissapear
            0.0
        });
    }
}

fn update_boost_meter(
    state: Res<GameState>,
    ui_scale: Res<UiScale>,
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

fn update_field(state: Res<GameState>, mut game_mode: ResMut<GameMode>, mut load_state: ResMut<NextState<LoadState>>) {
    if state.game_mode != *game_mode {
        *game_mode = state.game_mode;
        load_state.set(LoadState::Despawn);
    }
}

fn listen(socket: Res<Connection>, key: Res<Input<KeyCode>>, mut game_state: ResMut<GameState>) {
    let mut changed = false;
    if key.just_pressed(KeyCode::R) {
        changed = true;

        game_state.ball.pos = Vec3A::new(0., -2000., 1500.);
        game_state.ball.vel = Vec3A::new(50., 1500., 1.);
    }

    if changed {
        if let Err(e) = socket.0.send(&game_state.to_bytes()) {
            error!("Failed to send state setting packet: {e}");
        }
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
            .insert_resource(GameMode::default())
            .add_systems(
                Update,
                (
                    establish_connection.run_if(in_state(LoadState::Connect)),
                    (
                        (
                            step_arena,
                            (
                                (update_ball, update_car, update_pads, update_field)
                                    .run_if(|updated: Res<PacketUpdated>| updated.0),
                                (listen, update_boost_meter),
                            ),
                        )
                            .chain(),
                        update_time,
                    )
                        .run_if(in_state(LoadState::None)),
                ),
            );
    }
}
