use std::f32::consts::PI;

use bevy::{
    math::{Mat3A, Vec3A},
    prelude::*,
};
use rocketsim_rs::{
    cxx::UniquePtr,
    math::{RotMat, Vec3 as RVec},
    sim::{
        arena::Arena,
        ball::BallState,
        car::{CarConfig, Team},
        CarControls,
    },
    GameState,
};

#[derive(Component)]
struct Ball;

#[derive(Component)]
struct Car(u32);

#[derive(Resource, Default)]
struct State(GameState);

pub struct RocketSimPlugin;

trait ToBevyVec {
    fn to_bevy(self) -> Vec3;
}

impl ToBevyVec for RVec {
    fn to_bevy(self) -> Vec3 {
        Vec3::new(self.x, self.z, self.y)
    }
}

trait ToBevyMat {
    fn to_bevy(self) -> Quat;
}

impl ToBevyMat for RotMat {
    fn to_bevy(self) -> Quat {
        // In RocketSim, the Z axis is up, but in Bevy, the Z and Y axis are swapped
        // We also need to rotate 90 degrees around the X axis and 180 degrees around the Y axis
        let mat = Mat3A::from_axis_angle(Vec3::Y, PI) * Mat3A::from_axis_angle(Vec3::X, PI / 2.) * Mat3A::from(self) * Mat3A::from_cols(Vec3A::X, -Vec3A::Z, Vec3A::Y);
        Quat::from_mat3a(&mat)
    }
}

#[derive(Component)]
struct BoostPad;

fn setup_arena(
    mut commands: Commands,
    mut state: ResMut<State>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut arena: NonSendMut<UniquePtr<Arena>>,
) {
    arena.pin_mut().add_car(Team::BLUE, CarConfig::octane());
    arena.pin_mut().add_car(Team::BLUE, CarConfig::dominus());
    arena.pin_mut().add_car(Team::BLUE, CarConfig::merc());
    arena.pin_mut().add_car(Team::ORANGE, CarConfig::breakout());
    arena.pin_mut().add_car(Team::ORANGE, CarConfig::hybrid());
    arena.pin_mut().add_car(Team::ORANGE, CarConfig::plank());
    arena.pin_mut().set_ball(BallState {
        pos: RVec::new(0., -2000., 1500.),
        vel: RVec::new(0., 1500., 1.),
        ..default()
    });

    arena.pin_mut().set_goal_scored_callback(
        |arena, _, _| {
            arena.reset_to_random_kickoff(None);
        },
        0,
    );

    arena
        .pin_mut()
        .set_all_controls(
            (1..=6u32)
                .map(|i| {
                    (
                        i,
                        CarControls {
                            throttle: 1.,
                            boost: true,
                            ..default()
                        },
                    )
                })
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .unwrap();

    let game_state = arena.pin_mut().get_game_state();

    let mut ball_material = StandardMaterial::from(Color::rgb(0.3, 0.3, 0.3));
    ball_material.perceptual_roughness = 0.8;

    let ball_radius = arena.get_ball_radius();
    let ball_translation = Transform::from_translation(game_state.ball.pos.to_bevy());

    commands.spawn((
        Ball,
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::UVSphere { radius: ball_radius, ..default() })),
            material: materials.add(ball_material),
            transform: ball_translation,
            ..default()
        },
    ));

    for (id, team, state, config) in &game_state.cars {
        let hitbox = config.hitbox_size.to_bevy();
        let color = match team {
            Team::BLUE => Color::rgb(0.03, 0.09, 0.79),
            Team::ORANGE => Color::rgb(0.82, 0.42, 0.02),
        };

        commands.spawn((
            Car(*id),
            PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Box::new(hitbox.x, hitbox.y, hitbox.z))),
                material: materials.add(StandardMaterial::from(color)),
                transform: Transform::from_translation(state.pos.to_bevy()),
                ..default()
            },
        ));
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

    state.0 = game_state;
}

fn step_arena(time: Res<Time>, mut arena: NonSendMut<UniquePtr<Arena>>, mut state: ResMut<State>) {
    let current_ticks = arena.get_tick_count();
    let required_ticks = time.elapsed_seconds() * arena.get_tick_rate();
    let needs_simulation = required_ticks.floor() as u64 - current_ticks;

    if needs_simulation > 0 {
        // just in case something else updated state, set the variables in arena
        arena.pin_mut().set_ball(state.0.ball);
        for (id, _, state, _) in &state.0.cars {
            arena.pin_mut().set_car(*id, *state).unwrap();
        }
        for (i, state) in state.0.pads.iter().map(|pad| pad.state).enumerate() {
            arena.pin_mut().set_pad_state(i, state);
        }

        arena.pin_mut().step(needs_simulation as i32);
        state.0 = arena.pin_mut().get_game_state();
    }
}

fn update_ball(state: Res<State>, mut ball: Query<(&mut Transform, &Handle<StandardMaterial>), With<Ball>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let (mut transform, standard_material) = ball.single_mut();
    let new_pos = state.0.ball.pos.to_bevy();
    transform.translation = new_pos;

    let material = materials.get_mut(standard_material).unwrap();

    let amount = (transform.translation.z.abs() / 3500.).min(0.55);
    material.base_color = if new_pos.z > 0. {
        Color::rgb(amount.max(0.3), (amount * (2. / 3.)).max(0.3), 0.3)
    } else {
        Color::rgb(0.3, 0.3, amount.max(0.3))
    };
}

fn update_car(state: Res<State>, mut cars: Query<(&mut Transform, &Car)>) {
    for (mut transform, car) in cars.iter_mut() {
        let car_state = state.0.cars.iter().find(|&(id, _, _, _)| car.0 == *id).unwrap().2;
        transform.translation = car_state.pos.to_bevy();
        transform.rotation = car_state.rot_mat.to_bevy();
    }
}

fn update_pads(state: Res<State>, query: Query<&Handle<StandardMaterial>, With<BoostPad>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    for (pad, handle) in state.0.pads.iter().zip(query.iter()) {
        let material = materials.get_mut(handle).unwrap();
        material.base_color = if pad.state.is_active {
            Color::rgba(0.9, 0.9, 0.1, 0.6)
        } else {
            // make inactive pads grey and more transparent
            Color::rgba(0.5, 0.5, 0.5, 0.3)
        };
    }
}

fn listen(key: Res<Input<KeyCode>>, mut state: ResMut<State>) {
    if key.just_pressed(KeyCode::R) {
        state.0.ball.pos = RVec::new(0., -2000., 1500.);
        state.0.ball.vel = RVec::new(0., 1500., 1.);
    }
}

impl Plugin for RocketSimPlugin {
    fn build(&self, app: &mut App) {
        rocketsim_rs::init(None);

        app.insert_non_send_resource(Arena::default_standard())
            .insert_resource(State::default())
            .add_startup_system(setup_arena)
            .add_system(step_arena)
            .add_systems((update_ball, update_car, update_pads).after(step_arena).before(listen))
            .add_system(update_ball.run_if(|state: Res<State>| state.is_changed()))
            .add_system(update_car.run_if(|state: Res<State>| state.is_changed()))
            .add_system(update_pads.run_if(|state: Res<State>| state.is_changed()))
            .add_system(listen);
    }
}
