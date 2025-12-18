use super::options::MenuFocused;
use crate::{
    flat::rocketsim::{TileState, Vec3},
    udp::{Connection, GameStates, SendableUdp},
};
use ahash::AHashMap;
use bevy::prelude::*;
use bevy_egui::{EguiContext, EguiPrimaryContextPass, PrimaryEguiContext, egui};

pub struct StateSettingInterface;

impl Plugin for StateSettingInterface {
    fn build(&self, app: &mut App) {
        app.insert_resource(EnableBallInfo::default())
            .insert_resource(UserBallState::default())
            .insert_resource(EnableCarInfo::default())
            .insert_resource(UserCarStates::default())
            .insert_resource(EnablePadInfo::default())
            .insert_resource(UserPadStates::default())
            .insert_resource(EnableTileInfo::default())
            .insert_resource(UserTileStates::default())
            .add_message::<UserSetBallState>()
            .add_message::<UserSetCarState>()
            .add_message::<UserSetPadState>()
            .add_message::<UserSetTileState>()
            .add_systems(
                EguiPrimaryContextPass,
                (
                    update_ball_info.run_if(resource_equals(EnableBallInfo(true))),
                    update_car_info.run_if(|enable_menu: Res<EnableCarInfo>| !enable_menu.0.is_empty()),
                    update_boost_pad_info.run_if(|enable_menu: Res<EnablePadInfo>| !enable_menu.0.is_empty()),
                    update_tile_info.run_if(|enable_menu: Res<EnableTileInfo>| !enable_menu.0.is_empty()),
                )
                    .chain()
                    .run_if(resource_exists::<Connection>.and(resource_equals(MenuFocused::default()))),
            )
            .add_systems(
                Update,
                (
                    set_user_ball_state.run_if(on_message::<UserSetBallState>),
                    set_user_car_state.run_if(on_message::<UserSetCarState>),
                    set_user_pad_state.run_if(on_message::<UserSetPadState>),
                    set_user_tile_state.run_if(on_message::<UserSetTileState>),
                )
                    .run_if(resource_exists::<Connection>.and(resource_equals(MenuFocused::default()))),
            );
    }
}

fn set_f32_from_str(num: &mut f32, s: &str) {
    if let Ok(f) = s.parse() {
        *num = f;
    }
}

fn set_vec3_from_arr_str(vec: &mut Vec3, arr: &[String; 3]) {
    set_f32_from_str(&mut vec.x, &arr[0]);
    set_f32_from_str(&mut vec.y, &arr[1]);
    set_f32_from_str(&mut vec.z, &arr[2]);
}

fn set_half_bool_from_usize(b: &mut bool, i: usize) {
    if i != 0 {
        *b = false;
    }
}

fn set_bool_from_usize(b: &mut bool, i: usize) {
    if i != 0 {
        *b = i == 1;
    }
}

#[derive(Message)]
struct UserSetPadState(usize);

#[derive(Resource, PartialEq, Eq)]
pub struct EnablePadInfo(AHashMap<usize, bool>);

impl Default for EnablePadInfo {
    #[inline]
    fn default() -> Self {
        Self(AHashMap::with_capacity(48))
    }
}

impl EnablePadInfo {
    pub fn toggle(&mut self, id: usize) {
        if let Some(enabled) = self.0.get_mut(&id) {
            *enabled = !*enabled;
        } else {
            self.0.insert(id, true);
        }
    }
}

#[derive(Default)]
struct UserPadState {
    pub is_active: usize,
    pub timer: String,
}

#[derive(Resource)]
pub struct UserPadStates(AHashMap<usize, UserPadState>);

impl Default for UserPadStates {
    #[inline]
    fn default() -> Self {
        Self(AHashMap::with_capacity(48))
    }
}

impl UserPadStates {
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

fn update_boost_pad_info(
    mut context: Single<&mut EguiContext, With<PrimaryEguiContext>>,
    game_states: Res<GameStates>,
    mut enable_menu: ResMut<EnablePadInfo>,
    mut set_user_state: MessageWriter<UserSetPadState>,
    mut user_pads: ResMut<UserPadStates>,
) {
    const USER_BOOL_NAMES: [&str; 3] = ["", "True", "False"];

    let ctx = context.get_mut();

    let Some(pads) = game_states.current.pads.as_ref() else {
        return;
    };

    for (i, pad) in pads.iter().enumerate() {
        let Some(entry) = enable_menu.0.get_mut(&i) else {
            continue;
        };

        if !*entry {
            continue;
        }

        let user_pad = user_pads.0.entry(i).or_default();

        let title = format!("{}Boost pad {}", if pad.config.is_big { "(Large) " } else { "" }, i);
        egui::Window::new(title).open(entry).show(ctx, |ui| {
            ui.label(format!(
                "Position: [{:.0}, {:.0}, {:.0}]",
                pad.config.pos.x, pad.config.pos.y, pad.config.pos.z
            ));

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(format!("Is active: {}", pad.state.is_active));
                    egui::ComboBox::from_id_salt("Is active").width(60.).show_index(
                        ui,
                        &mut user_pad.is_active,
                        USER_BOOL_NAMES.len(),
                        |i| USER_BOOL_NAMES[i],
                    );
                });
                ui.vertical(|ui| {
                    ui.label(format!("Timer: {:.1}", pad.state.cooldown));
                    ui.add(egui::TextEdit::singleline(&mut user_pad.timer).desired_width(60.));
                });
            });

            if ui
                .button("     Set all     ")
                .on_hover_text("Set all (defined) boost pad properties")
                .clicked()
            {
                set_user_state.write(UserSetPadState(i));
            }
        });
    }
}

fn set_user_pad_state(
    mut events: MessageReader<UserSetPadState>,
    mut game_states: ResMut<GameStates>,
    user_pads: Res<UserPadStates>,
    socket: Res<Connection>,
) {
    for event in events.read() {
        let Some(user_pad) = user_pads.0.get(&event.0) else {
            continue;
        };

        let (is_active, cooldown) = {
            let Some(current_pads) = game_states.current.pads.as_mut() else {
                continue;
            };

            let pad = &mut current_pads[event.0];

            set_bool_from_usize(&mut pad.state.is_active, user_pad.is_active);
            set_f32_from_str(&mut pad.state.cooldown, &user_pad.timer);

            (pad.state.is_active, pad.state.cooldown)
        };

        if let Some(next_pads) = game_states.next.pads.as_mut() {
            let pad = &mut next_pads[event.0];
            pad.state.is_active = is_active;
            pad.state.cooldown = cooldown;
        }
    }

    socket.send(SendableUdp::State(game_states.next.clone())).unwrap();
}

#[derive(Message)]
struct UserSetBallState(SetBallStateAmount);

#[derive(Resource, Default, PartialEq, Eq)]
pub struct EnableBallInfo(bool);

impl EnableBallInfo {
    pub fn toggle(&mut self) {
        self.0 = !self.0;
    }
}

#[derive(Default, Resource)]
struct UserBallState {
    pub pos: [String; 3],
    pub vel: [String; 3],
    pub ang_vel: [String; 3],
    pub hs_y_target_dir: String,
    pub ds_accum_hit_force: String,
    pub ds_y_target_dir: String,
}

enum SetBallStateAmount {
    Pos,
    Vel,
    AngVel,
    Heatseeker,
    Dropshot,
    All,
}

fn set_user_ball_state(
    mut events: MessageReader<UserSetBallState>,
    mut game_states: ResMut<GameStates>,
    user_ball: Res<UserBallState>,
    socket: Res<Connection>,
) {
    for event in events.read() {
        match event.0 {
            SetBallStateAmount::Pos => {
                set_vec3_from_arr_str(&mut game_states.current.ball.physics.pos, &user_ball.pos);
                game_states.next.ball.physics.pos = game_states.current.ball.physics.pos;
            }
            SetBallStateAmount::Vel => {
                set_vec3_from_arr_str(&mut game_states.current.ball.physics.vel, &user_ball.vel);
                game_states.next.ball.physics.vel = game_states.current.ball.physics.vel;
            }
            SetBallStateAmount::AngVel => {
                set_vec3_from_arr_str(&mut game_states.current.ball.physics.ang_vel, &user_ball.ang_vel);
                game_states.next.ball.physics.ang_vel = game_states.current.ball.physics.ang_vel;
            }
            SetBallStateAmount::Heatseeker => {
                set_f32_from_str(&mut game_states.current.ball.hs_info.y_target_dir, &user_ball.hs_y_target_dir);
                game_states.next.ball.hs_info.y_target_dir = game_states.current.ball.hs_info.y_target_dir;
            }
            SetBallStateAmount::Dropshot => {
                set_f32_from_str(
                    &mut game_states.current.ball.ds_info.accumulated_hit_force,
                    &user_ball.ds_accum_hit_force,
                );

                let force = game_states.current.ball.ds_info.accumulated_hit_force;
                game_states.next.ball.ds_info.accumulated_hit_force = force;

                if force >= 11000. {
                    game_states.current.ball.ds_info.charge_level = 3;
                    game_states.next.ball.ds_info.charge_level = 3;
                } else if force >= 2500. {
                    game_states.current.ball.ds_info.charge_level = 2;
                    game_states.next.ball.ds_info.charge_level = 2;
                } else {
                    game_states.current.ball.ds_info.charge_level = 1;
                    game_states.next.ball.ds_info.charge_level = 1;
                }

                set_f32_from_str(&mut game_states.current.ball.ds_info.y_target_dir, &user_ball.ds_y_target_dir);
                game_states.next.ball.ds_info.y_target_dir = game_states.current.ball.ds_info.y_target_dir;
            }
            SetBallStateAmount::All => {
                set_vec3_from_arr_str(&mut game_states.current.ball.physics.pos, &user_ball.pos);
                game_states.next.ball.physics.pos = game_states.current.ball.physics.pos;

                set_vec3_from_arr_str(&mut game_states.current.ball.physics.vel, &user_ball.vel);
                game_states.next.ball.physics.vel = game_states.current.ball.physics.vel;

                set_vec3_from_arr_str(&mut game_states.current.ball.physics.ang_vel, &user_ball.ang_vel);
                game_states.next.ball.physics.ang_vel = game_states.current.ball.physics.ang_vel;

                // heatseekr
                set_f32_from_str(&mut game_states.current.ball.hs_info.y_target_dir, &user_ball.hs_y_target_dir);
                game_states.next.ball.hs_info.y_target_dir = game_states.current.ball.hs_info.y_target_dir;

                // dropshot
                set_f32_from_str(
                    &mut game_states.current.ball.ds_info.accumulated_hit_force,
                    &user_ball.ds_accum_hit_force,
                );

                let force = game_states.current.ball.ds_info.accumulated_hit_force;
                game_states.next.ball.ds_info.accumulated_hit_force = force;

                if force >= 2500. {
                    game_states.current.ball.ds_info.charge_level = 2;
                    game_states.next.ball.ds_info.charge_level = 2;
                } else if force >= 11000. {
                    game_states.current.ball.ds_info.charge_level = 3;
                    game_states.next.ball.ds_info.charge_level = 3;
                } else {
                    game_states.current.ball.ds_info.charge_level = 1;
                    game_states.next.ball.ds_info.charge_level = 1;
                }

                set_f32_from_str(&mut game_states.current.ball.ds_info.y_target_dir, &user_ball.ds_y_target_dir);
                game_states.next.ball.ds_info.y_target_dir = game_states.current.ball.ds_info.y_target_dir;
            }
        }
    }

    socket.send(SendableUdp::State(game_states.next.clone())).unwrap();
}

fn update_ball_info(
    mut context: Single<&mut EguiContext, With<PrimaryEguiContext>>,
    game_states: Res<GameStates>,
    mut enable_menu: ResMut<EnableBallInfo>,
    mut set_user_state: MessageWriter<UserSetBallState>,
    mut user_ball: ResMut<UserBallState>,
) {
    let ctx = context.get_mut();

    egui::Window::new("Ball").open(&mut enable_menu.0).show(ctx, |ui| {
        ui.label(format!(
            "Position: [{:.1}, {:.1}, {:.1}]",
            game_states.current.ball.physics.pos.x,
            game_states.current.ball.physics.pos.y,
            game_states.current.ball.physics.pos.z
        ));
        ui.horizontal(|ui| {
            ui.label("X: ");
            ui.add(egui::TextEdit::singleline(&mut user_ball.pos[0]).desired_width(50.));
            ui.label("Y: ");
            ui.add(egui::TextEdit::singleline(&mut user_ball.pos[1]).desired_width(50.));
            ui.label("Z: ");
            ui.add(egui::TextEdit::singleline(&mut user_ball.pos[2]).desired_width(50.));
            if ui.button("Set").on_hover_text("Set ball position").clicked() {
                set_user_state.write(UserSetBallState(SetBallStateAmount::Pos));
            }
        });
        ui.label(format!(
            "Velocity: [{:.1}, {:.1}, {:.1}]",
            game_states.current.ball.physics.vel.x,
            game_states.current.ball.physics.vel.y,
            game_states.current.ball.physics.vel.z
        ));
        ui.horizontal(|ui| {
            ui.label("X: ");
            ui.add(egui::TextEdit::singleline(&mut user_ball.vel[0]).desired_width(50.));
            ui.label("Y: ");
            ui.add(egui::TextEdit::singleline(&mut user_ball.vel[1]).desired_width(50.));
            ui.label("Z: ");
            ui.add(egui::TextEdit::singleline(&mut user_ball.vel[2]).desired_width(50.));
            if ui.button("Set").on_hover_text("Set ball velocity").clicked() {
                set_user_state.write(UserSetBallState(SetBallStateAmount::Vel));
            }
        });
        ui.label(format!(
            "Angular velocity: [{:.1}, {:.1}, {:.1}]",
            game_states.current.ball.physics.ang_vel.x,
            game_states.current.ball.physics.ang_vel.y,
            game_states.current.ball.physics.ang_vel.z
        ));
        ui.horizontal(|ui| {
            ui.label("X: ");
            ui.add(egui::TextEdit::singleline(&mut user_ball.ang_vel[0]).desired_width(50.));
            ui.label("Y: ");
            ui.add(egui::TextEdit::singleline(&mut user_ball.ang_vel[1]).desired_width(50.));
            ui.label("Z: ");
            ui.add(egui::TextEdit::singleline(&mut user_ball.ang_vel[2]).desired_width(50.));
            if ui.button("Set").on_hover_text("Set ball angular velocity").clicked() {
                set_user_state.write(UserSetBallState(SetBallStateAmount::AngVel));
            }
        });

        ui.add_space(10.0);
        ui.label("Heatseeker info:");
        ui.horizontal(|ui| {
            ui.label(format!(
                "Current target speed: {:.1}",
                game_states.current.ball.hs_info.cur_target_speed
            ));

            ui.label(format!(
                "Time since hit: {:.1}",
                game_states.current.ball.hs_info.time_since_hit
            ));
        });
        ui.horizontal(|ui| {
            ui.label(format!(
                "Y target direction: {:.1} - ",
                game_states.current.ball.hs_info.y_target_dir
            ));
            ui.add(egui::TextEdit::singleline(&mut user_ball.hs_y_target_dir).desired_width(50.));
        });
        ui.horizontal(|ui| {
            ui.label(format!(
                "Target speed: {:.1}",
                game_states.current.ball.hs_info.cur_target_speed
            ));
            ui.label(format!(
                "Time since hit: {:.1}",
                game_states.current.ball.hs_info.time_since_hit
            ));
        });
        if ui.button("Set").on_hover_text("Set heatseeker info").clicked() {
            set_user_state.write(UserSetBallState(SetBallStateAmount::Heatseeker));
        }

        ui.add_space(10.0);
        ui.label("Dropshot info:");
        ui.horizontal(|ui| {
            ui.label(format!(
                "Accumulated hit force: {:.1} - ",
                game_states.current.ball.ds_info.accumulated_hit_force
            ));
            ui.add(egui::TextEdit::singleline(&mut user_ball.ds_accum_hit_force).desired_width(50.));
        });
        ui.horizontal(|ui| {
            ui.label(format!(
                "Y target direction: {:.1} - ",
                game_states.current.ball.ds_info.y_target_dir
            ));
            ui.add(egui::TextEdit::singleline(&mut user_ball.ds_y_target_dir).desired_width(50.));
        });
        ui.horizontal(|ui| {
            ui.label(format!("Charge level: {:.1}", game_states.current.ball.ds_info.charge_level));
            ui.label(format!("Has damaged: {}", game_states.current.ball.ds_info.has_damaged));
            ui.label(format!(
                "Last damage tick: {}",
                game_states.current.ball.ds_info.last_damage_tick
            ));
        });
        if ui.button("Set").on_hover_text("Set dropshot info").clicked() {
            set_user_state.write(UserSetBallState(SetBallStateAmount::Dropshot));
        }

        ui.add_space(10.0);
        if ui
            .button("     Set all     ")
            .on_hover_text("Set all (defined) ball properties")
            .clicked()
        {
            set_user_state.write(UserSetBallState(SetBallStateAmount::All));
        }
    });
}

#[derive(Message)]
struct UserSetTileState((usize, usize), SetTileStateAmount);

#[derive(Resource, PartialEq, Eq)]
pub struct EnableTileInfo(AHashMap<(usize, usize), bool>);

impl Default for EnableTileInfo {
    #[inline]
    fn default() -> Self {
        Self(AHashMap::new())
    }
}

impl EnableTileInfo {
    pub fn toggle(&mut self, id: (usize, usize)) {
        if let Some(enabled) = self.0.get_mut(&id) {
            *enabled = !*enabled;
        } else {
            self.0.insert(id, true);
        }
    }
}

#[derive(Default)]
struct UserTileState {
    pub damage_state: usize,
}

#[derive(Resource)]
pub struct UserTileStates(AHashMap<(usize, usize), UserTileState>);

impl Default for UserTileStates {
    #[inline]
    fn default() -> Self {
        Self(AHashMap::new())
    }
}

impl UserTileStates {
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

enum SetTileStateAmount {
    All,
}

fn set_user_tile_state(
    mut events: MessageReader<UserSetTileState>,
    mut game_states: ResMut<GameStates>,
    user_tiles: Res<UserTileStates>,
    socket: Res<Connection>,
) {
    for event in events.read() {
        let team = event.0.0;
        let index = event.0.1;

        let Some(user_tile) = user_tiles.0.get(&event.0) else {
            continue;
        };

        let state = TileState::try_from(user_tile.damage_state as u8).unwrap();

        match event.1 {
            SetTileStateAmount::All => {
                if let Some(current_tiles) = game_states.current.tiles.as_mut() {
                    let current_tiles = [&mut current_tiles.blue_tiles, &mut current_tiles.orange_tiles];
                    current_tiles[team][index].state = state;
                }

                if let Some(next_tiles) = game_states.next.tiles.as_mut() {
                    let next_tiles = [&mut next_tiles.blue_tiles, &mut next_tiles.orange_tiles];
                    next_tiles[team][index].state = state;
                };
            }
        }
    }

    socket.send(SendableUdp::State(game_states.next.clone())).unwrap();
}

fn update_tile_info(
    mut context: Single<&mut EguiContext, With<PrimaryEguiContext>>,
    game_states: Res<GameStates>,
    mut enable_menu: ResMut<EnableTileInfo>,
    mut set_user_state: MessageWriter<UserSetTileState>,
    mut user_tile: ResMut<UserTileStates>,
) {
    const TEAM_NAMES: [&str; 2] = ["Blue", "Orange"];
    const USER_DAMAGE_NAMES: [&str; 3] = ["Full", "Damaged", "Broken"];

    let Some(tiles) = game_states.current.tiles.as_ref() else {
        return;
    };

    let ctx = context.get_mut();

    for (team, team_tiles) in [&tiles.blue_tiles, &tiles.orange_tiles].iter().enumerate() {
        for (index, tile) in team_tiles.iter().enumerate() {
            let id = (team, index);
            let Some(entry) = enable_menu.0.get_mut(&id) else {
                continue;
            };

            if !*entry {
                continue;
            }

            let user_tile = user_tile.0.entry(id).or_default();

            egui::Window::new(format!("{} Tile #{}", TEAM_NAMES[team], index))
                .open(entry)
                .show(ctx, |ui| {
                    ui.label(format!("Damage state: {}", USER_DAMAGE_NAMES[tile.state as usize]));
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("Damage state").width(60.).show_index(
                            ui,
                            &mut user_tile.damage_state,
                            USER_DAMAGE_NAMES.len(),
                            |i| USER_DAMAGE_NAMES[i],
                        );

                        if ui.button("Set").on_hover_text("Set tile damage state").clicked() {
                            set_user_state.write(UserSetTileState(id, SetTileStateAmount::All));
                        }
                    });
                });
        }
    }
}

#[derive(Message)]
struct UserSetCarState(u64, SetCarStateAmount);

#[derive(Resource, PartialEq, Eq)]
pub struct EnableCarInfo(AHashMap<u64, bool>);

impl Default for EnableCarInfo {
    #[inline]
    fn default() -> Self {
        Self(AHashMap::with_capacity(8))
    }
}

impl EnableCarInfo {
    pub fn toggle(&mut self, id: u64) {
        if let Some(enabled) = self.0.get_mut(&id) {
            *enabled = !*enabled;
        } else {
            self.0.insert(id, true);
        }
    }
}

#[derive(Default)]
struct UserCarState {
    pub pos: [String; 3],
    pub vel: [String; 3],
    pub ang_vel: [String; 3],
    pub has_jumped: usize,
    pub has_double_jumped: usize,
    pub has_flipped: usize,
    pub boost: String,
    pub demo_respawn_timer: String,
}

#[derive(Resource)]
pub struct UserCarStates(AHashMap<u64, UserCarState>);

impl Default for UserCarStates {
    #[inline]
    fn default() -> Self {
        Self(AHashMap::with_capacity(8))
    }
}

impl UserCarStates {
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn remove(&mut self, id: u64) {
        self.0.remove(&id);
    }
}

enum SetCarStateAmount {
    Pos,
    Vel,
    AngVel,
    Jumped,
    DoubleJumped,
    Flipped,
    Boost,
    DemoRespawnTimer,
    All,
}

fn set_user_car_state(
    mut events: MessageReader<UserSetCarState>,
    mut game_states: ResMut<GameStates>,
    user_cars: Res<UserCarStates>,
    socket: Res<Connection>,
) {
    if game_states.current.cars.is_none() || game_states.next.cars.is_none() {
        return;
    }

    for event in events.read() {
        let Some(car_index) = game_states
            .current
            .cars
            .as_ref()
            .unwrap()
            .iter()
            .position(|car| car.id == event.0)
        else {
            continue;
        };
        let Some(user_car) = user_cars.0.get(&event.0) else {
            continue;
        };

        match event.1 {
            SetCarStateAmount::Pos => {
                set_vec3_from_arr_str(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.physics.pos,
                    &user_car.pos,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.physics.pos =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.physics.pos;
            }
            SetCarStateAmount::Vel => {
                set_vec3_from_arr_str(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.physics.vel,
                    &user_car.vel,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.physics.vel =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.physics.vel;
            }
            SetCarStateAmount::AngVel => {
                set_vec3_from_arr_str(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.physics.ang_vel,
                    &user_car.ang_vel,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.physics.ang_vel =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.physics.ang_vel;
            }
            SetCarStateAmount::Jumped => {
                set_half_bool_from_usize(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.has_jumped,
                    user_car.has_jumped,
                );
                set_half_bool_from_usize(
                    &mut game_states.next.cars.as_mut().unwrap()[car_index].state.has_jumped,
                    user_car.has_jumped,
                );
            }
            SetCarStateAmount::DoubleJumped => {
                set_half_bool_from_usize(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.has_double_jumped,
                    user_car.has_double_jumped,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.has_double_jumped =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.has_double_jumped;
            }
            SetCarStateAmount::Flipped => {
                set_half_bool_from_usize(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.has_flipped,
                    user_car.has_flipped,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.has_flipped =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.has_flipped;
            }
            SetCarStateAmount::Boost => {
                set_f32_from_str(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.boost,
                    &user_car.boost,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.boost =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.boost;
            }
            SetCarStateAmount::DemoRespawnTimer => {
                set_f32_from_str(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.demo_respawn_timer,
                    &user_car.demo_respawn_timer,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.demo_respawn_timer =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.demo_respawn_timer;

                if game_states.current.cars.as_mut().unwrap()[car_index].state.demo_respawn_timer != 0. {
                    game_states.current.cars.as_mut().unwrap()[car_index].state.is_demoed = true;
                    game_states.next.cars.as_mut().unwrap()[car_index].state.is_demoed = true;
                }
            }
            SetCarStateAmount::All => {
                set_vec3_from_arr_str(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.physics.pos,
                    &user_car.pos,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.physics.pos =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.physics.pos;

                set_vec3_from_arr_str(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.physics.vel,
                    &user_car.vel,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.physics.vel =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.physics.vel;

                set_vec3_from_arr_str(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.physics.ang_vel,
                    &user_car.ang_vel,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.physics.ang_vel =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.physics.ang_vel;

                set_half_bool_from_usize(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.has_jumped,
                    user_car.has_jumped,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.has_jumped =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.has_jumped;

                set_half_bool_from_usize(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.has_double_jumped,
                    user_car.has_double_jumped,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.has_double_jumped =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.has_double_jumped;

                set_half_bool_from_usize(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.has_flipped,
                    user_car.has_flipped,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.has_flipped =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.has_flipped;

                set_f32_from_str(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.boost,
                    &user_car.boost,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.boost =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.boost;

                set_f32_from_str(
                    &mut game_states.current.cars.as_mut().unwrap()[car_index].state.demo_respawn_timer,
                    &user_car.demo_respawn_timer,
                );
                game_states.next.cars.as_mut().unwrap()[car_index].state.demo_respawn_timer =
                    game_states.current.cars.as_mut().unwrap()[car_index].state.demo_respawn_timer;

                if game_states.current.cars.as_mut().unwrap()[car_index].state.demo_respawn_timer != 0. {
                    game_states.current.cars.as_mut().unwrap()[car_index].state.is_demoed = true;
                    game_states.next.cars.as_mut().unwrap()[car_index].state.is_demoed = true;
                }
            }
        }
    }

    socket.send(SendableUdp::State(game_states.next.clone())).unwrap();
}

fn update_car_info(
    mut context: Single<&mut EguiContext, With<PrimaryEguiContext>>,
    game_states: Res<GameStates>,
    mut enable_menu: ResMut<EnableCarInfo>,
    mut set_user_state: MessageWriter<UserSetCarState>,
    mut user_cars: ResMut<UserCarStates>,
) {
    const USER_BOOL_NAMES: [&str; 2] = ["", "False"];

    let Some(cars) = game_states.current.cars.as_ref() else {
        return;
    };

    let ctx = context.get_mut();

    for car in cars {
        let Some(entry) = enable_menu.0.get_mut(&car.id) else {
            continue;
        };

        if !*entry {
            continue;
        }

        let user_car = user_cars.0.entry(car.id).or_default();

        egui::Window::new(format!("{:?} Car {}", car.team, car.id))
            .open(entry)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(format!("Is on ground: {}", car.state.is_on_ground));
                        ui.label(format!("Jump time: {:.1}", car.state.jump_time));
                        ui.label(format!("Flip time: {:.1}", car.state.flip_time));
                        ui.label(format!("Is flipping: {}", car.state.is_flipping));
                        ui.label(format!("Is jumping: {}", car.state.is_jumping));
                        ui.label(format!("Is jumping: {}", car.state.is_jumping));
                        ui.label(format!("Boosting time: {:.1}", car.state.boosting_time));
                        ui.label(format!("Is supersonic: {}", car.state.is_supersonic));
                        ui.label(format!("Supersonic time: {:.1}", car.state.supersonic_time));
                        ui.label(format!("Handbrake val: {:.1}", car.state.handbrake_val));
                        ui.label(format!("Is auto flipping: {}", car.state.is_auto_flipping));
                        ui.label(format!("Auto flip timer: {:.1}", car.state.auto_flip_timer));
                        ui.label(format!("Is demolished: {}", car.state.is_demoed));
                    });

                    ui.vertical(|ui| {
                        ui.label(format!(
                            "Position: [{:.1}, {:.1}, {:.1}]",
                            car.state.physics.pos.x, car.state.physics.pos.y, car.state.physics.pos.z
                        ));
                        ui.horizontal(|ui| {
                            ui.label("X: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.pos[0]).desired_width(50.));
                            ui.label("Y: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.pos[1]).desired_width(50.));
                            ui.label("Z: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.pos[2]).desired_width(50.));
                            if ui.button("Set").on_hover_text("Set car position").clicked() {
                                set_user_state.write(UserSetCarState(car.id, SetCarStateAmount::Pos));
                            }
                        });

                        ui.label(format!(
                            "Velocity: [{:.1}, {:.1}, {:.1}]",
                            car.state.physics.vel.x, car.state.physics.vel.y, car.state.physics.vel.z
                        ));
                        ui.horizontal(|ui| {
                            ui.label("X: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.vel[0]).desired_width(50.));
                            ui.label("Y: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.vel[1]).desired_width(50.));
                            ui.label("Z: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.vel[2]).desired_width(50.));
                            if ui.button("Set").on_hover_text("Set car velocity").clicked() {
                                set_user_state.write(UserSetCarState(car.id, SetCarStateAmount::Vel));
                            }
                        });

                        ui.label(format!(
                            "Angular velocity: [{:.1}, {:.1}, {:.1}]",
                            car.state.physics.ang_vel.x, car.state.physics.ang_vel.y, car.state.physics.ang_vel.z
                        ));
                        ui.horizontal(|ui| {
                            ui.label("X: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.ang_vel[0]).desired_width(50.));
                            ui.label("Y: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.ang_vel[1]).desired_width(50.));
                            ui.label("Z: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.ang_vel[2]).desired_width(50.));
                            if ui.button("Set").on_hover_text("Set car angular velocity").clicked() {
                                set_user_state.write(UserSetCarState(car.id, SetCarStateAmount::AngVel));
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(format!("Has jumped: {}", car.state.has_jumped));
                                ui.horizontal(|ui| {
                                    egui::ComboBox::from_id_salt("Has jumped").width(60.).show_index(
                                        ui,
                                        &mut user_car.has_jumped,
                                        USER_BOOL_NAMES.len(),
                                        |i| USER_BOOL_NAMES[i],
                                    );

                                    if ui.button("Set").on_hover_text("Set car has jumped").clicked() {
                                        set_user_state.write(UserSetCarState(car.id, SetCarStateAmount::Jumped));
                                    }
                                });

                                ui.label(format!("Has flipped: {}", car.state.has_flipped));
                                ui.horizontal(|ui| {
                                    egui::ComboBox::from_id_salt("Has flipped").width(60.).show_index(
                                        ui,
                                        &mut user_car.has_flipped,
                                        USER_BOOL_NAMES.len(),
                                        |i| USER_BOOL_NAMES[i],
                                    );

                                    if ui.button("Set").on_hover_text("Set car has flipped").clicked() {
                                        set_user_state.write(UserSetCarState(car.id, SetCarStateAmount::Flipped));
                                    }
                                });

                                ui.label("");

                                if ui
                                    .button("     Set all     ")
                                    .on_hover_text("Set all (defined) car properties")
                                    .clicked()
                                {
                                    set_user_state.write(UserSetCarState(car.id, SetCarStateAmount::All));
                                }
                            });
                            ui.vertical(|ui| {
                                ui.label(format!("Has double jumped: {}", car.state.has_double_jumped));
                                ui.horizontal(|ui| {
                                    egui::ComboBox::from_id_salt("Has double jumped").width(60.).show_index(
                                        ui,
                                        &mut user_car.has_double_jumped,
                                        USER_BOOL_NAMES.len(),
                                        |i| USER_BOOL_NAMES[i],
                                    );

                                    if ui.button("Set").on_hover_text("Set car has double jumped").clicked() {
                                        set_user_state.write(UserSetCarState(car.id, SetCarStateAmount::DoubleJumped));
                                    }
                                });

                                ui.label(format!("Boost: {:.0}", car.state.boost));
                                ui.horizontal(|ui| {
                                    ui.add(egui::TextEdit::singleline(&mut user_car.boost).desired_width(60.));
                                    if ui.button("Set").on_hover_text("Set car boost").clicked() {
                                        set_user_state.write(UserSetCarState(car.id, SetCarStateAmount::Boost));
                                    }
                                });

                                ui.label(format!("Demo respawn timer: {:.1}", car.state.demo_respawn_timer));
                                ui.horizontal(|ui| {
                                    ui.add(egui::TextEdit::singleline(&mut user_car.demo_respawn_timer).desired_width(60.));
                                    if ui.button("Set").on_hover_text("Set car demo respawn timer").clicked() {
                                        set_user_state.write(UserSetCarState(car.id, SetCarStateAmount::DemoRespawnTimer));
                                    }
                                });
                            });
                        });
                    });
                });

                ui.vertical(|ui| {
                    ui.label("Last known controls:");
                    ui.horizontal(|ui| {
                        ui.label(format!("Throttle: {:.1}", car.state.last_controls.throttle));
                        ui.label(format!("Steer: {:.1}", car.state.last_controls.steer));
                        ui.label(format!("Boost: {}", car.state.last_controls.boost));
                        ui.label(format!("Handbrake: {}", car.state.last_controls.handbrake));
                    });
                    ui.horizontal(|ui| {
                        ui.label(format!("Pitch: {:.1}", car.state.last_controls.pitch));
                        ui.label(format!("Yaw: {:.1}", car.state.last_controls.yaw));
                        ui.label(format!("Roll: {:.1}", car.state.last_controls.roll));
                        ui.label(format!("Jump: {}", car.state.last_controls.jump));
                    });
                });
            });
    }
}
