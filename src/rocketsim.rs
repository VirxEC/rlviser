use bevy::prelude::*;
use rocketsim_rs::{
    cxx::UniquePtr,
    glam_ext::{glam::Vec3A, BallA, CarA, GameStateA},
    math::Vec3 as RVec,
    sim::{
        arena::Arena,
        ball::BallState,
        car::{CarConfig, Team},
    },
};

#[derive(Component)]
pub struct Ball(BallA);

#[derive(Component)]
pub struct Car(CarA);

#[derive(Resource, Default)]
pub struct State(GameStateA);

pub struct RocketSimPlugin;

trait ToBevy {
    fn to_bevy(self) -> Self;
}

impl ToBevy for Vec3A {
    fn to_bevy(self) -> Self {
        Self::new(self.x, self.z, self.y)
    }
}

fn setup_arena(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut arena: NonSendMut<UniquePtr<Arena>>) {
    arena.pin_mut().add_car(Team::BLUE, CarConfig::octane());
    arena.pin_mut().add_car(Team::ORANGE, CarConfig::octane());
    arena.pin_mut().set_ball(BallState {
        pos: RVec::new(0., 0., 1500.),
        vel: RVec::new(0., 0., 1.),
        ..default()
    });

    arena.pin_mut().set_goal_scored_callback(
        |arena, _, _| {
            arena.reset_to_random_kickoff(None);
        },
        0,
    );

    let game_state = arena.pin_mut().get_game_state().to_glam();

    commands.spawn((
        Ball(game_state.ball),
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::UVSphere {
                radius: arena.get_ball_radius(),
                ..default()
            })),
            material: materials.add(StandardMaterial::from(Color::rgb(0.95, 0.16, 0.45))),
            transform: Transform::from_translation(game_state.ball.pos.to_bevy().into()),
            ..default()
        },
    ));
}

fn step_arena(time: Res<Time>, mut arena: NonSendMut<UniquePtr<Arena>>, mut state: ResMut<State>) {
    let current_ticks = arena.get_tick_count();
    let required_ticks = time.elapsed_seconds() * arena.get_tick_rate();
    let needs_simulation = required_ticks.floor() as u64 - current_ticks;

    if needs_simulation > 0 {
        arena.pin_mut().step(needs_simulation as i32);
        state.0 = arena.pin_mut().get_game_state().to_glam();
    }
}

fn use_game_state(state: Res<State>, mut ball: Query<&mut Transform, With<Ball>>) {
    ball.single_mut().translation = state.0.ball.pos.to_bevy().into()
}

impl Plugin for RocketSimPlugin {
    fn build(&self, app: &mut App) {
        rocketsim_rs::init(None);

        app.insert_non_send_resource(Arena::default_standard())
            .insert_resource(State::default())
            .add_startup_system(setup_arena)
            .add_system(step_arena.before(use_game_state))
            .add_system(use_game_state.run_if(|state: Res<State>| state.is_changed()));
    }
}
