use super::options::MenuFocused;
use crate::{
    morton::Morton,
    udp::{Connection, GameStates, SendableUdp},
};
use bevy::{math::Vec3A, prelude::*, utils::HashMap};
use bevy_egui::{egui, EguiContexts};

pub struct StateSettingInterface;

impl Plugin for StateSettingInterface {
    fn build(&self, app: &mut App) {
        app.insert_resource(EnableBallInfo::default())
            .insert_resource(UserBallState::default())
            .insert_resource(EnableCarInfo::default())
            .insert_resource(UserCarStates::default())
            .insert_resource(EnablePadInfo::default())
            .insert_resource(UserPadStates::default())
            .add_event::<UserSetBallState>()
            .add_event::<UserSetCarState>()
            .add_event::<UserSetPadState>()
            .add_systems(
                Update,
                (
                    update_ball_info.run_if(resource_equals(EnableBallInfo(true))),
                    update_car_info.run_if(|enable_menu: Res<EnableCarInfo>| !enable_menu.0.is_empty()),
                    update_boost_pad_info.run_if(|enable_menu: Res<EnablePadInfo>| !enable_menu.0.is_empty()),
                    (
                        set_user_ball_state.run_if(on_event::<UserSetBallState>),
                        set_user_car_state.run_if(on_event::<UserSetCarState>),
                        set_user_pad_state.run_if(on_event::<UserSetPadState>),
                    )
                        .run_if(resource_exists::<Connection>),
                )
                    .run_if(resource_equals(MenuFocused::default())),
            );
    }
}

fn set_f32_from_str(num: &mut f32, s: &str) {
    if let Ok(f) = s.parse() {
        *num = f;
    }
}

fn set_vec3_from_arr_str(vec: &mut Vec3A, arr: &[String; 3]) {
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

#[derive(Event)]
struct UserSetPadState(u64);

#[derive(Resource, PartialEq, Eq)]
pub struct EnablePadInfo(HashMap<u64, bool>);

impl Default for EnablePadInfo {
    #[inline]
    fn default() -> Self {
        Self(HashMap::with_capacity(48))
    }
}

impl EnablePadInfo {
    pub fn toggle(&mut self, id: u64) {
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
pub struct UserPadStates(HashMap<u64, UserPadState>);

impl Default for UserPadStates {
    #[inline]
    fn default() -> Self {
        Self(HashMap::with_capacity(48))
    }
}

impl UserPadStates {
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

fn update_boost_pad_info(
    mut contexts: EguiContexts,
    game_states: Res<GameStates>,
    mut enable_menu: ResMut<EnablePadInfo>,
    mut set_user_state: EventWriter<UserSetPadState>,
    mut user_pads: ResMut<UserPadStates>,
) {
    const USER_BOOL_NAMES: [&str; 3] = ["", "True", "False"];

    let ctx = contexts.ctx_mut();

    let morton_generator = Morton::default();
    for (i, pad) in game_states.current.pads.iter().enumerate() {
        let code = morton_generator.get_code(pad.position);
        let Some(entry) = enable_menu.0.get_mut(&code) else {
            continue;
        };

        if !*entry {
            continue;
        }

        let user_pad = user_pads.0.entry(code).or_default();

        let title = format!("{}Boost pad {}", if pad.is_big { "(Large) " } else { "" }, i);
        egui::Window::new(title).open(entry).show(ctx, |ui| {
            ui.label(format!(
                "Position: [{:.0}, {:.0}, {:.0}]",
                pad.position.x, pad.position.y, pad.position.z
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
                set_user_state.send(UserSetPadState(code));
            }
        });
    }
}

fn set_user_pad_state(
    mut events: EventReader<UserSetPadState>,
    mut game_states: ResMut<GameStates>,
    user_pads: Res<UserPadStates>,
    socket: Res<Connection>,
) {
    let morton_generator = Morton::default();
    let mut sorted_pads = game_states
        .current
        .pads
        .iter()
        .enumerate()
        .map(|(i, pad)| (i, morton_generator.get_code(pad.position)))
        .collect::<Vec<_>>();
    radsort::sort_by_key(&mut sorted_pads, |(_, code)| *code);

    for event in events.read() {
        let Some(user_pad) = user_pads.0.get(&event.0) else {
            continue;
        };

        let Ok(index) = sorted_pads.binary_search_by_key(&event.0, |(_, code)| *code) else {
            continue;
        };

        let (is_active, cooldown) = {
            let pad = &mut game_states.current.pads[sorted_pads[index].0];

            set_bool_from_usize(&mut pad.state.is_active, user_pad.is_active);
            set_f32_from_str(&mut pad.state.cooldown, &user_pad.timer);

            (pad.state.is_active, pad.state.cooldown)
        };

        let pad = &mut game_states.next.pads[sorted_pads[index].0];
        pad.state.is_active = is_active;
        pad.state.cooldown = cooldown;
    }

    socket.send(SendableUdp::State(game_states.next.clone())).unwrap();
}

#[derive(Event)]
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
}

enum SetBallStateAmount {
    Pos,
    Vel,
    AngVel,
    All,
}

fn set_user_ball_state(
    mut events: EventReader<UserSetBallState>,
    mut game_states: ResMut<GameStates>,
    user_ball: Res<UserBallState>,
    socket: Res<Connection>,
) {
    for event in events.read() {
        match event.0 {
            SetBallStateAmount::Pos => {
                set_vec3_from_arr_str(&mut game_states.current.ball.pos, &user_ball.pos);
                game_states.next.ball.pos = game_states.current.ball.pos;
            }
            SetBallStateAmount::Vel => {
                set_vec3_from_arr_str(&mut game_states.current.ball.vel, &user_ball.vel);
                game_states.next.ball.vel = game_states.current.ball.vel;
            }
            SetBallStateAmount::AngVel => {
                set_vec3_from_arr_str(&mut game_states.current.ball.ang_vel, &user_ball.ang_vel);
                game_states.next.ball.ang_vel = game_states.current.ball.ang_vel;
            }
            SetBallStateAmount::All => {
                set_vec3_from_arr_str(&mut game_states.current.ball.pos, &user_ball.pos);
                game_states.next.ball.pos = game_states.current.ball.pos;

                set_vec3_from_arr_str(&mut game_states.current.ball.vel, &user_ball.vel);
                game_states.next.ball.vel = game_states.current.ball.vel;

                set_vec3_from_arr_str(&mut game_states.current.ball.ang_vel, &user_ball.ang_vel);
                game_states.next.ball.ang_vel = game_states.current.ball.ang_vel;
            }
        }
    }

    socket.send(SendableUdp::State(game_states.next.clone())).unwrap();
}

fn update_ball_info(
    mut contexts: EguiContexts,
    game_states: Res<GameStates>,
    mut enable_menu: ResMut<EnableBallInfo>,
    mut set_user_state: EventWriter<UserSetBallState>,
    mut user_ball: ResMut<UserBallState>,
) {
    egui::Window::new("Ball")
        .open(&mut enable_menu.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.label(format!(
                "Position: [{:.1}, {:.1}, {:.1}]",
                game_states.current.ball.pos.x, game_states.current.ball.pos.y, game_states.current.ball.pos.z
            ));
            ui.horizontal(|ui| {
                ui.label("X: ");
                ui.add(egui::TextEdit::singleline(&mut user_ball.pos[0]).desired_width(50.));
                ui.label("Y: ");
                ui.add(egui::TextEdit::singleline(&mut user_ball.pos[1]).desired_width(50.));
                ui.label("Z: ");
                ui.add(egui::TextEdit::singleline(&mut user_ball.pos[2]).desired_width(50.));
                if ui.button("Set").on_hover_text("Set ball position").clicked() {
                    set_user_state.send(UserSetBallState(SetBallStateAmount::Pos));
                }
            });
            ui.label(format!(
                "Velocity: [{:.1}, {:.1}, {:.1}]",
                game_states.current.ball.vel.x, game_states.current.ball.vel.y, game_states.current.ball.vel.z
            ));
            ui.horizontal(|ui| {
                ui.label("X: ");
                ui.add(egui::TextEdit::singleline(&mut user_ball.vel[0]).desired_width(50.));
                ui.label("Y: ");
                ui.add(egui::TextEdit::singleline(&mut user_ball.vel[1]).desired_width(50.));
                ui.label("Z: ");
                ui.add(egui::TextEdit::singleline(&mut user_ball.vel[2]).desired_width(50.));
                if ui.button("Set").on_hover_text("Set ball velocity").clicked() {
                    set_user_state.send(UserSetBallState(SetBallStateAmount::Vel));
                }
            });
            ui.label(format!(
                "Angular velocity: [{:.1}, {:.1}, {:.1}]",
                game_states.current.ball.ang_vel.x, game_states.current.ball.ang_vel.y, game_states.current.ball.ang_vel.z
            ));
            ui.horizontal(|ui| {
                ui.label("X: ");
                ui.add(egui::TextEdit::singleline(&mut user_ball.ang_vel[0]).desired_width(50.));
                ui.label("Y: ");
                ui.add(egui::TextEdit::singleline(&mut user_ball.ang_vel[1]).desired_width(50.));
                ui.label("Z: ");
                ui.add(egui::TextEdit::singleline(&mut user_ball.ang_vel[2]).desired_width(50.));
                if ui.button("Set").on_hover_text("Set ball angular velocity").clicked() {
                    set_user_state.send(UserSetBallState(SetBallStateAmount::AngVel));
                }
            });
            if ui
                .button("     Set all     ")
                .on_hover_text("Set all (defined) ball properties")
                .clicked()
            {
                set_user_state.send(UserSetBallState(SetBallStateAmount::All));
            }
        });
}

#[derive(Event)]
struct UserSetCarState(u32, SetCarStateAmount);

#[derive(Resource, PartialEq, Eq)]
pub struct EnableCarInfo(HashMap<u32, bool>);

impl Default for EnableCarInfo {
    #[inline]
    fn default() -> Self {
        Self(HashMap::with_capacity(8))
    }
}

impl EnableCarInfo {
    pub fn toggle(&mut self, id: u32) {
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
pub struct UserCarStates(HashMap<u32, UserCarState>);

impl Default for UserCarStates {
    #[inline]
    fn default() -> Self {
        Self(HashMap::with_capacity(8))
    }
}

impl UserCarStates {
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn remove(&mut self, id: u32) {
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
    mut events: EventReader<UserSetCarState>,
    mut game_states: ResMut<GameStates>,
    user_cars: Res<UserCarStates>,
    socket: Res<Connection>,
) {
    for event in events.read() {
        let Some(car_index) = game_states.current.cars.iter().position(|car| car.id == event.0) else {
            continue;
        };
        let Some(user_car) = user_cars.0.get(&event.0) else {
            continue;
        };

        match event.1 {
            SetCarStateAmount::Pos => {
                set_vec3_from_arr_str(&mut game_states.current.cars[car_index].state.pos, &user_car.pos);
                game_states.next.cars[car_index].state.pos = game_states.current.cars[car_index].state.pos;
            }
            SetCarStateAmount::Vel => {
                set_vec3_from_arr_str(&mut game_states.current.cars[car_index].state.vel, &user_car.vel);
                game_states.next.cars[car_index].state.vel = game_states.current.cars[car_index].state.vel;
            }
            SetCarStateAmount::AngVel => {
                set_vec3_from_arr_str(&mut game_states.current.cars[car_index].state.ang_vel, &user_car.ang_vel);
                game_states.next.cars[car_index].state.ang_vel = game_states.current.cars[car_index].state.ang_vel;
            }
            SetCarStateAmount::Jumped => {
                set_half_bool_from_usize(&mut game_states.current.cars[car_index].state.has_jumped, user_car.has_jumped);
                set_half_bool_from_usize(&mut game_states.next.cars[car_index].state.has_jumped, user_car.has_jumped);
            }
            SetCarStateAmount::DoubleJumped => {
                set_half_bool_from_usize(
                    &mut game_states.current.cars[car_index].state.has_double_jumped,
                    user_car.has_double_jumped,
                );
                game_states.next.cars[car_index].state.has_double_jumped =
                    game_states.current.cars[car_index].state.has_double_jumped;
            }
            SetCarStateAmount::Flipped => {
                set_half_bool_from_usize(
                    &mut game_states.current.cars[car_index].state.has_flipped,
                    user_car.has_flipped,
                );
                game_states.next.cars[car_index].state.has_flipped = game_states.current.cars[car_index].state.has_flipped;
            }
            SetCarStateAmount::Boost => {
                set_f32_from_str(&mut game_states.current.cars[car_index].state.boost, &user_car.boost);
                game_states.next.cars[car_index].state.boost = game_states.current.cars[car_index].state.boost;
            }
            SetCarStateAmount::DemoRespawnTimer => {
                set_f32_from_str(
                    &mut game_states.current.cars[car_index].state.demo_respawn_timer,
                    &user_car.demo_respawn_timer,
                );
                game_states.next.cars[car_index].state.demo_respawn_timer =
                    game_states.current.cars[car_index].state.demo_respawn_timer;

                if game_states.current.cars[car_index].state.demo_respawn_timer != 0. {
                    game_states.current.cars[car_index].state.is_demoed = true;
                    game_states.next.cars[car_index].state.is_demoed = true;
                }
            }
            SetCarStateAmount::All => {
                set_vec3_from_arr_str(&mut game_states.current.cars[car_index].state.pos, &user_car.pos);
                game_states.next.cars[car_index].state.pos = game_states.current.cars[car_index].state.pos;

                set_vec3_from_arr_str(&mut game_states.current.cars[car_index].state.vel, &user_car.vel);
                game_states.next.cars[car_index].state.vel = game_states.current.cars[car_index].state.vel;

                set_vec3_from_arr_str(&mut game_states.current.cars[car_index].state.ang_vel, &user_car.ang_vel);
                game_states.next.cars[car_index].state.ang_vel = game_states.current.cars[car_index].state.ang_vel;

                set_half_bool_from_usize(&mut game_states.current.cars[car_index].state.has_jumped, user_car.has_jumped);
                game_states.next.cars[car_index].state.has_jumped = game_states.current.cars[car_index].state.has_jumped;

                set_half_bool_from_usize(
                    &mut game_states.current.cars[car_index].state.has_double_jumped,
                    user_car.has_double_jumped,
                );
                game_states.next.cars[car_index].state.has_double_jumped =
                    game_states.current.cars[car_index].state.has_double_jumped;

                set_half_bool_from_usize(
                    &mut game_states.current.cars[car_index].state.has_flipped,
                    user_car.has_flipped,
                );
                game_states.next.cars[car_index].state.has_flipped = game_states.current.cars[car_index].state.has_flipped;

                set_f32_from_str(&mut game_states.current.cars[car_index].state.boost, &user_car.boost);
                game_states.next.cars[car_index].state.boost = game_states.current.cars[car_index].state.boost;

                set_f32_from_str(
                    &mut game_states.current.cars[car_index].state.demo_respawn_timer,
                    &user_car.demo_respawn_timer,
                );
                game_states.next.cars[car_index].state.demo_respawn_timer =
                    game_states.current.cars[car_index].state.demo_respawn_timer;

                if game_states.current.cars[car_index].state.demo_respawn_timer != 0. {
                    game_states.current.cars[car_index].state.is_demoed = true;
                    game_states.next.cars[car_index].state.is_demoed = true;
                }
            }
        }
    }

    socket.send(SendableUdp::State(game_states.next.clone())).unwrap();
}

fn update_car_info(
    mut contexts: EguiContexts,
    game_states: Res<GameStates>,
    mut enable_menu: ResMut<EnableCarInfo>,
    mut set_user_state: EventWriter<UserSetCarState>,
    mut user_cars: ResMut<UserCarStates>,
) {
    const USER_BOOL_NAMES: [&str; 2] = ["", "False"];

    let ctx = contexts.ctx_mut();

    for car in game_states.current.cars.iter() {
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
                        ui.label(format!("Time spent boosting: {:.1}", car.state.time_spent_boosting));
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
                            car.state.pos.x, car.state.pos.y, car.state.pos.z
                        ));
                        ui.horizontal(|ui| {
                            ui.label("X: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.pos[0]).desired_width(50.));
                            ui.label("Y: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.pos[1]).desired_width(50.));
                            ui.label("Z: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.pos[2]).desired_width(50.));
                            if ui.button("Set").on_hover_text("Set car position").clicked() {
                                set_user_state.send(UserSetCarState(car.id, SetCarStateAmount::Pos));
                            }
                        });

                        ui.label(format!(
                            "Velocity: [{:.1}, {:.1}, {:.1}]",
                            car.state.vel.x, car.state.vel.y, car.state.vel.z
                        ));
                        ui.horizontal(|ui| {
                            ui.label("X: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.vel[0]).desired_width(50.));
                            ui.label("Y: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.vel[1]).desired_width(50.));
                            ui.label("Z: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.vel[2]).desired_width(50.));
                            if ui.button("Set").on_hover_text("Set car velocity").clicked() {
                                set_user_state.send(UserSetCarState(car.id, SetCarStateAmount::Vel));
                            }
                        });

                        ui.label(format!(
                            "Angular velocity: [{:.1}, {:.1}, {:.1}]",
                            car.state.ang_vel.x, car.state.ang_vel.y, car.state.ang_vel.z
                        ));
                        ui.horizontal(|ui| {
                            ui.label("X: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.ang_vel[0]).desired_width(50.));
                            ui.label("Y: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.ang_vel[1]).desired_width(50.));
                            ui.label("Z: ");
                            ui.add(egui::TextEdit::singleline(&mut user_car.ang_vel[2]).desired_width(50.));
                            if ui.button("Set").on_hover_text("Set car angular velocity").clicked() {
                                set_user_state.send(UserSetCarState(car.id, SetCarStateAmount::AngVel));
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
                                        set_user_state.send(UserSetCarState(car.id, SetCarStateAmount::Jumped));
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
                                        set_user_state.send(UserSetCarState(car.id, SetCarStateAmount::Flipped));
                                    }
                                });

                                ui.label("");

                                if ui
                                    .button("     Set all     ")
                                    .on_hover_text("Set all (defined) car properties")
                                    .clicked()
                                {
                                    set_user_state.send(UserSetCarState(car.id, SetCarStateAmount::All));
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
                                        set_user_state.send(UserSetCarState(car.id, SetCarStateAmount::DoubleJumped));
                                    }
                                });

                                ui.label(format!("Boost: {:.0}", car.state.boost));
                                ui.horizontal(|ui| {
                                    ui.add(egui::TextEdit::singleline(&mut user_car.boost).desired_width(60.));
                                    if ui.button("Set").on_hover_text("Set car boost").clicked() {
                                        set_user_state.send(UserSetCarState(car.id, SetCarStateAmount::Boost));
                                    }
                                });

                                ui.label(format!("Demo respawn timer: {:.1}", car.state.demo_respawn_timer));
                                ui.horizontal(|ui| {
                                    ui.add(egui::TextEdit::singleline(&mut user_car.demo_respawn_timer).desired_width(60.));
                                    if ui.button("Set").on_hover_text("Set car demo respawn timer").clicked() {
                                        set_user_state.send(UserSetCarState(car.id, SetCarStateAmount::DemoRespawnTimer));
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
