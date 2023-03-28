use bevy::prelude::*;
use rocketsim_rs::{
    cxx::UniquePtr,
    sim::{
        arena::Arena,
        car::{CarConfig, Team},
    },
};

pub struct RocketSimPlugin;

fn setup_arena(mut arena: NonSendMut<UniquePtr<Arena>>) {
    arena.pin_mut().add_car(Team::BLUE, CarConfig::octane());
    arena.pin_mut().add_car(Team::ORANGE, CarConfig::octane());

    arena.pin_mut().set_goal_scored_callback(|arena, _| {
        arena.reset_to_random_kickoff(None);
    });
}

fn step_arena(mut arena: NonSendMut<UniquePtr<Arena>>, time: Res<Time>) {
    let current_ticks = arena.get_tick_count();
    let required_ticks = time.elapsed_seconds() * arena.get_tick_rate();
    let needs_simulation = required_ticks.floor() as u64 - current_ticks;

    arena.pin_mut().step(needs_simulation as i32);
}

impl Plugin for RocketSimPlugin {
    fn build(&self, app: &mut App) {
        rocketsim_rs::init(None);

        app.insert_non_send_resource(Arena::default_standard())
            .add_startup_system(setup_arena)
            .add_system(step_arena);
    }
}
