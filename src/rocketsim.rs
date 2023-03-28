use bevy::prelude::*;
use rocketsim_rs::{
    cxx::UniquePtr,
    glam_ext::GameStateA,
    sim::{
        arena::Arena,
        car::{CarConfig, Team},
    },
};

#[derive(Resource, Default)]
pub struct State(GameStateA);

pub struct RocketSimPlugin;

fn setup_arena(mut arena: NonSendMut<UniquePtr<Arena>>) {
    arena.pin_mut().add_car(Team::BLUE, CarConfig::octane());
    arena.pin_mut().add_car(Team::ORANGE, CarConfig::octane());

    arena.pin_mut().set_goal_scored_callback(|arena, _| {
        arena.reset_to_random_kickoff(None);
    });
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

fn use_game_state(_state: Res<State>) {
    // todo!
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
