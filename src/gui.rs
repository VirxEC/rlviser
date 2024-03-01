use crate::{
    bytes::ToBytes,
    camera::{DaylightOffset, PrimaryCamera, Sun},
    morton::Morton,
    rocketsim::GameState,
    udp::Connection,
};
use ahash::AHashMap;
use bevy::{
    math::Vec3A,
    pbr::DirectionalLightShadowMap,
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bevy_framepace::{FramepaceSettings, Limiter};
use bevy_mod_picking::picking_core::PickingPluginsSettings;
use std::{
    fs,
    io::{self, Write},
};

#[cfg(debug_assertions)]
use crate::camera::{EntityName, HighlightedEntity};

pub struct DebugOverlayPlugin;

#[derive(Resource)]
pub struct BallCam {
    pub enabled: bool,
}

impl Default for BallCam {
    #[inline]
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Resource)]
pub struct ShowTime {
    pub enabled: bool,
}

impl Default for ShowTime {
    #[inline]
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Resource)]
pub struct UiScale {
    pub scale: f32,
}

impl Default for UiScale {
    #[inline]
    fn default() -> Self {
        Self { scale: 1. }
    }
}

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .insert_resource(if cfg!(feature = "ssao") { Msaa::Off } else { Msaa::default() })
            .insert_resource(BallCam::default())
            .insert_resource(UiScale::default())
            .insert_resource(ShowTime::default())
            .insert_resource(Options::default_read_file())
            .insert_resource(MenuFocused::default())
            .insert_resource(EnableBallInfo::default())
            .insert_resource(UserBallState::default())
            .insert_resource(EnableCarInfo::default())
            .insert_resource(UserCarStates::default())
            .insert_resource(EnablePadInfo::default())
            .insert_resource(UserPadStates::default())
            .insert_resource(PickingPluginsSettings {
                enable: true,
                enable_input: false,
                enable_highlighting: false,
                enable_interacting: true,
            })
            .add_event::<UserSetBallState>()
            .add_event::<UserSetCarState>()
            .add_event::<UserSetPadState>()
            .add_systems(
                Update,
                (
                    listen,
                    (
                        ui_system,
                        toggle_vsync,
                        toggle_ballcam,
                        toggle_show_time,
                        update_daytime,
                        #[cfg(not(feature = "ssao"))]
                        update_msaa,
                        update_ui_scale,
                        update_shadows,
                        update_ball_info.run_if(resource_equals(EnableBallInfo(true))),
                        update_car_info.run_if(|enable_menu: Res<EnableCarInfo>| !enable_menu.0.is_empty()),
                        update_boost_pad_info.run_if(|enable_menu: Res<EnablePadInfo>| !enable_menu.0.is_empty()),
                        set_user_ball_state.run_if(on_event::<UserSetBallState>()),
                        set_user_car_state.run_if(on_event::<UserSetCarState>()),
                        set_user_pad_state.run_if(on_event::<UserSetPadState>()),
                    )
                        .run_if(resource_equals(MenuFocused(true))),
                    update_camera_state,
                    write_settings_to_file,
                )
                    .chain(),
            );

        #[cfg(debug_assertions)]
        app.add_systems(Update, debug_ui);
    }
}

#[derive(Resource, Default, PartialEq, Eq)]
pub struct EnableBallInfo(bool);

impl EnableBallInfo {
    pub fn toggle(&mut self) {
        self.0 = !self.0;
    }
}

#[derive(Resource, PartialEq, Eq)]
pub struct EnableCarInfo(AHashMap<u32, bool>);

impl Default for EnableCarInfo {
    #[inline]
    fn default() -> Self {
        Self(AHashMap::with_capacity(8))
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

#[derive(Resource, PartialEq, Eq)]
pub struct EnablePadInfo(AHashMap<u64, bool>);

impl Default for EnablePadInfo {
    #[inline]
    fn default() -> Self {
        Self(AHashMap::with_capacity(48))
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
pub struct UserPadStates(AHashMap<u64, UserPadState>);

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

#[derive(Event)]
struct UserSetPadState(u64);

fn update_boost_pad_info(
    mut contexts: EguiContexts,
    game_state: Res<GameState>,
    mut enable_menu: ResMut<EnablePadInfo>,
    mut set_user_state: EventWriter<UserSetPadState>,
    mut user_pads: ResMut<UserPadStates>,
) {
    const USER_BOOL_NAMES: [&str; 3] = ["", "True", "False"];

    let ctx = contexts.ctx_mut();

    let morton_generator = Morton::default();
    for (i, pad) in game_state.pads.iter().enumerate() {
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
                    egui::ComboBox::from_id_source("Is active").width(60.).show_index(
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
    mut game_state: ResMut<GameState>,
    user_pads: Res<UserPadStates>,
    socket: Res<Connection>,
) {
    let morton_generator = Morton::default();
    let mut sorted_pads = game_state
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
        let pad = &mut game_state.pads[sorted_pads[index].0];

        set_bool_from_usize(&mut pad.state.is_active, user_pad.is_active);
        set_f32_from_str(&mut pad.state.cooldown, &user_pad.timer);
    }

    if let Err(e) = socket.0.send(&game_state.to_bytes()) {
        error!("Failed to send boost pad information: {e}");
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
pub struct UserCarStates(AHashMap<u32, UserCarState>);

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

#[derive(Event)]
struct UserSetCarState(u32, SetCarStateAmount);

fn set_user_car_state(
    mut events: EventReader<UserSetCarState>,
    mut game_state: ResMut<GameState>,
    user_cars: Res<UserCarStates>,
    socket: Res<Connection>,
) {
    for event in events.read() {
        let Some(car_index) = game_state.cars.iter().position(|car| car.id == event.0) else {
            continue;
        };
        let Some(user_car) = user_cars.0.get(&event.0) else {
            continue;
        };

        match event.1 {
            SetCarStateAmount::Pos => set_vec3_from_arr_str(&mut game_state.cars[car_index].state.pos, &user_car.pos),
            SetCarStateAmount::Vel => set_vec3_from_arr_str(&mut game_state.cars[car_index].state.vel, &user_car.vel),
            SetCarStateAmount::AngVel => {
                set_vec3_from_arr_str(&mut game_state.cars[car_index].state.ang_vel, &user_car.ang_vel);
            }
            SetCarStateAmount::Jumped => {
                set_half_bool_from_usize(&mut game_state.cars[car_index].state.has_jumped, user_car.has_jumped);
            }
            SetCarStateAmount::DoubleJumped => set_half_bool_from_usize(
                &mut game_state.cars[car_index].state.has_double_jumped,
                user_car.has_double_jumped,
            ),
            SetCarStateAmount::Flipped => {
                set_half_bool_from_usize(&mut game_state.cars[car_index].state.has_flipped, user_car.has_flipped);
            }
            SetCarStateAmount::Boost => set_f32_from_str(&mut game_state.cars[car_index].state.boost, &user_car.boost),
            SetCarStateAmount::DemoRespawnTimer => set_f32_from_str(
                &mut game_state.cars[car_index].state.demo_respawn_timer,
                &user_car.demo_respawn_timer,
            ),
            SetCarStateAmount::All => {
                set_vec3_from_arr_str(&mut game_state.cars[car_index].state.pos, &user_car.pos);
                set_vec3_from_arr_str(&mut game_state.cars[car_index].state.vel, &user_car.vel);
                set_vec3_from_arr_str(&mut game_state.cars[car_index].state.ang_vel, &user_car.ang_vel);
                set_half_bool_from_usize(&mut game_state.cars[car_index].state.has_jumped, user_car.has_jumped);
                set_half_bool_from_usize(
                    &mut game_state.cars[car_index].state.has_double_jumped,
                    user_car.has_double_jumped,
                );
                set_half_bool_from_usize(&mut game_state.cars[car_index].state.has_flipped, user_car.has_flipped);
                set_f32_from_str(&mut game_state.cars[car_index].state.boost, &user_car.boost);
                set_f32_from_str(
                    &mut game_state.cars[car_index].state.demo_respawn_timer,
                    &user_car.demo_respawn_timer,
                );
            }
        }
    }

    if let Err(e) = socket.0.send(&game_state.to_bytes()) {
        error!("Failed to send car information: {e}");
    }
}

fn update_car_info(
    mut contexts: EguiContexts,
    game_state: Res<GameState>,
    mut enable_menu: ResMut<EnableCarInfo>,
    mut set_user_state: EventWriter<UserSetCarState>,
    mut user_cars: ResMut<UserCarStates>,
) {
    const USER_BOOL_NAMES: [&str; 2] = ["", "False"];

    let ctx = contexts.ctx_mut();

    for car in game_state.cars.iter() {
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
                                    egui::ComboBox::from_id_source("Has jumped").width(60.).show_index(
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
                                    egui::ComboBox::from_id_source("Has flipped").width(60.).show_index(
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
                                    egui::ComboBox::from_id_source("Has double jumped").width(60.).show_index(
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

#[derive(Event)]
struct UserSetBallState(SetBallStateAmount);

fn update_ball_info(
    mut contexts: EguiContexts,
    game_state: Res<GameState>,
    mut enable_menu: ResMut<EnableBallInfo>,
    mut set_user_state: EventWriter<UserSetBallState>,
    mut user_ball: ResMut<UserBallState>,
) {
    egui::Window::new("Ball")
        .open(&mut enable_menu.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.label(format!(
                "Position: [{:.1}, {:.1}, {:.1}]",
                game_state.ball.pos.x, game_state.ball.pos.y, game_state.ball.pos.z
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
                game_state.ball.vel.x, game_state.ball.vel.y, game_state.ball.vel.z
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
                game_state.ball.ang_vel.x, game_state.ball.ang_vel.y, game_state.ball.ang_vel.z
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

fn set_user_ball_state(
    mut events: EventReader<UserSetBallState>,
    mut game_state: ResMut<GameState>,
    user_ball: Res<UserBallState>,
    socket: Res<Connection>,
) {
    for event in events.read() {
        match event.0 {
            SetBallStateAmount::Pos => set_vec3_from_arr_str(&mut game_state.ball.pos, &user_ball.pos),
            SetBallStateAmount::Vel => set_vec3_from_arr_str(&mut game_state.ball.vel, &user_ball.vel),
            SetBallStateAmount::AngVel => set_vec3_from_arr_str(&mut game_state.ball.ang_vel, &user_ball.ang_vel),
            SetBallStateAmount::All => {
                set_vec3_from_arr_str(&mut game_state.ball.pos, &user_ball.pos);
                set_vec3_from_arr_str(&mut game_state.ball.vel, &user_ball.vel);
                set_vec3_from_arr_str(&mut game_state.ball.ang_vel, &user_ball.ang_vel);
            }
        }
    }

    if let Err(e) = socket.0.send(&game_state.to_bytes()) {
        error!("Failed to send ball information: {e}");
    }
}

#[derive(Resource, PartialEq, Eq)]
struct MenuFocused(pub bool);

impl Default for MenuFocused {
    #[inline]
    fn default() -> Self {
        Self(true)
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Resource)]
struct Options {
    vsync: bool,
    uncap_fps: bool,
    fps_limit: f64,
    fps: (usize, [f32; 120]),
    ball_cam: bool,
    stop_day: bool,
    daytime: f32,
    day_speed: f32,
    msaa: usize,
    camera_state: PrimaryCamera,
    show_time: bool,
    ui_scale: f32,
    shadows: usize,
}

impl Default for Options {
    #[inline]
    fn default() -> Self {
        Self {
            vsync: false,
            uncap_fps: false,
            fps_limit: 120.,
            fps: (0, [0.; 120]),
            ball_cam: true,
            stop_day: true,
            daytime: 25.,
            day_speed: 1.,
            msaa: 2,
            camera_state: PrimaryCamera::Spectator,
            show_time: true,
            ui_scale: 1.,
            shadows: 0,
        }
    }
}

impl Options {
    const FILE_NAME: &'static str = "settings.txt";

    #[inline]
    fn default_read_file() -> Self {
        Self::read_from_file().unwrap_or_else(|_| Self::create_file_from_defualt())
    }

    fn read_from_file() -> io::Result<Self> {
        let mut options = Self::default();

        let file = fs::read_to_string(Self::FILE_NAME)?;

        for line in file.lines() {
            let mut parts = line.split('=');

            let Some(key) = parts.next() else {
                continue;
            };

            let Some(value) = parts.next() else {
                continue;
            };

            match key {
                "vsync" => options.vsync = value.parse().unwrap(),
                "uncap_fps" => options.uncap_fps = value.parse().unwrap(),
                "fps_limit" => options.fps_limit = value.parse().unwrap(),
                "ball_cam" => options.ball_cam = value.parse().unwrap(),
                "stop_day" => options.stop_day = value.parse().unwrap(),
                "daytime" => options.daytime = value.parse().unwrap(),
                "day_speed" => options.day_speed = value.parse().unwrap(),
                "msaa" => options.msaa = value.parse().unwrap(),
                "camera_state" => options.camera_state = serde_json::from_str(value).unwrap(),
                "show_time" => options.show_time = value.parse().unwrap(),
                "ui_scale" => options.ui_scale = value.parse().unwrap(),
                "shadows" => options.shadows = value.parse().unwrap(),
                _ => println!("Unknown key {key} with value {value}"),
            }
        }

        Ok(options)
    }

    fn create_file_from_defualt() -> Self {
        let options = Self::default();

        if let Err(e) = options.write_options_to_file() {
            println!("Failed to create {} due to: {e}", Self::FILE_NAME);
        }

        options
    }

    fn write_options_to_file(&self) -> io::Result<()> {
        let mut file = fs::File::create(Self::FILE_NAME)?;

        file.write_fmt(format_args!("vsync={}\n", self.vsync))?;
        file.write_fmt(format_args!("uncap_fps={}\n", self.uncap_fps))?;
        file.write_fmt(format_args!("fps_limit={}\n", self.fps_limit))?;
        file.write_fmt(format_args!("ball_cam={}\n", self.ball_cam))?;
        file.write_fmt(format_args!("stop_day={}\n", self.stop_day))?;
        file.write_fmt(format_args!("daytime={}\n", self.daytime))?;
        file.write_fmt(format_args!("day_speed={}\n", self.day_speed))?;
        file.write_fmt(format_args!("msaa={}\n", self.msaa))?;
        file.write_fmt(format_args!("camera_state={}\n", serde_json::to_string(&self.camera_state)?))?;
        file.write_fmt(format_args!("show_time={}\n", self.show_time))?;
        file.write_fmt(format_args!("ui_scale={}\n", self.ui_scale))?;
        file.write_fmt(format_args!("shadows={}\n", self.shadows))?;

        Ok(())
    }

    #[inline]
    #[allow(clippy::float_cmp)]
    fn is_not_similar(&self, other: &Self) -> bool {
        self.vsync != other.vsync
            || self.uncap_fps != other.uncap_fps
            || self.fps_limit != other.fps_limit
            || self.ball_cam != other.ball_cam
            || self.stop_day != other.stop_day
            || self.daytime != other.daytime
            || self.day_speed != other.day_speed
            || self.msaa != other.msaa
            || self.camera_state != other.camera_state
            || self.show_time != other.show_time
            || self.ui_scale != other.ui_scale
            || self.shadows != other.shadows
    }
}

#[cfg(debug_assertions)]
fn debug_ui(
    mut contexts: EguiContexts,
    heq: Query<(&Transform, &EntityName), With<HighlightedEntity>>,
    cam_pos: Query<&Transform, With<PrimaryCamera>>,
) {
    let ctx = contexts.ctx_mut();
    let camera_pos = cam_pos.single().translation;

    let (he_pos, highlighted_entity_name) = heq
        .get_single()
        .map(|(transform, he)| (transform.translation, he.name.clone()))
        .unwrap_or((Vec3::default(), Box::from("None")));

    egui::Window::new("Debug").show(ctx, |ui| {
        ui.label(format!(
            "Primary camera position: [{:.0}, {:.0}, {:.0}]",
            camera_pos.x, camera_pos.y, camera_pos.z
        ));
        ui.label(format!("HE position: [{:.0}, {:.0}, {:.0}]", he_pos.x, he_pos.y, he_pos.z));
        ui.label(format!("Highlighted entity: {highlighted_entity_name}"));
    });
}

fn ui_system(
    mut menu_focused: ResMut<MenuFocused>,
    mut options: ResMut<Options>,
    mut contexts: EguiContexts,
    time: Res<Time>,
) {
    #[cfg(not(feature = "ssao"))]
    const MSAA_NAMES: [&str; 4] = ["Off", "2x", "4x", "8x"];
    const SHADOW_NAMES: [&str; 4] = ["Off", "0.5x", "1x", "1.5x"];
    let ctx = contexts.ctx_mut();

    let dt = time.delta_seconds();
    if dt == 0.0 {
        return;
    }

    let (i, history) = &mut options.fps;

    history[*i] = dt;
    *i += 1;
    *i %= history.len();

    let avg_dt = history.iter().sum::<f32>() / history.len() as f32;
    let fps = 1. / avg_dt;

    egui::Window::new("Menu")
        .auto_sized()
        .open(&mut menu_focused.0)
        .show(ctx, |ui| {
            ui.label(format!("FPS: {fps:.0}"));
            ui.horizontal(|ui| {
                ui.checkbox(&mut options.vsync, "vsync");
                ui.checkbox(&mut options.uncap_fps, "Uncap FPS");
                ui.add(egui::DragValue::new(&mut options.fps_limit).speed(5.).clamp_range(30..=600));
            });

            ui.horizontal(|ui| {
                egui::ComboBox::from_label("Shadows").width(50.).show_index(
                    ui,
                    &mut options.shadows,
                    SHADOW_NAMES.len(),
                    |i| SHADOW_NAMES[i],
                );
                #[cfg(not(feature = "ssao"))]
                egui::ComboBox::from_label("MSAA")
                    .width(40.)
                    .show_index(ui, &mut options.msaa, MSAA_NAMES.len(), |i| MSAA_NAMES[i]);
            });

            ui.add_space(10.);

            ui.horizontal(|ui| {
                ui.checkbox(&mut options.show_time, "In-game time");
                ui.checkbox(&mut options.ball_cam, "Ball cam");
            });
            ui.add(egui::Slider::new(&mut options.ui_scale, 0.4..=4.0).text("UI scale"));

            ui.add_space(10.);

            ui.checkbox(&mut options.stop_day, "Stop day cycle");
            ui.add(egui::Slider::new(&mut options.daytime, 0.0..=150.0).text("Daytime"));
            ui.add(egui::Slider::new(&mut options.day_speed, 0.0..=10.0).text("Day speed"));
        });
}

fn update_shadows(
    options: Res<Options>,
    mut query: Query<&mut DirectionalLight, With<Sun>>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
) {
    query.single_mut().shadows_enabled = options.shadows != 0;
    shadow_map.size = 2048
        * match options.shadows {
            2 => 2,
            3 => 3,
            _ => 1,
        };
}

fn toggle_ballcam(options: Res<Options>, mut ballcam: ResMut<BallCam>) {
    ballcam.enabled = options.ball_cam;
}

fn toggle_vsync(options: Res<Options>, mut framepace: ResMut<FramepaceSettings>) {
    framepace.limiter = if options.vsync {
        Limiter::Auto
    } else if options.uncap_fps {
        Limiter::Off
    } else {
        Limiter::from_framerate(options.fps_limit)
    };
}

#[cfg(not(feature = "ssao"))]
fn update_msaa(options: Res<Options>, mut msaa: ResMut<Msaa>) {
    const MSAA_SAMPLES: [u32; 4] = [1, 2, 4, 8];
    if MSAA_SAMPLES[options.msaa] == msaa.samples() {
        return;
    }

    *msaa = match options.msaa {
        0 => Msaa::Off,
        1 => Msaa::Sample2,
        2 => Msaa::Sample4,
        3 => Msaa::Sample8,
        _ => unreachable!(),
    };
}

fn toggle_show_time(options: Res<Options>, mut show_time: ResMut<ShowTime>) {
    show_time.enabled = options.show_time;
}

fn update_ui_scale(options: Res<Options>, mut ui_scale: ResMut<UiScale>) {
    if options.ui_scale == ui_scale.scale {
        return;
    }

    ui_scale.scale = options.ui_scale;
}

fn update_daytime(options: Res<Options>, mut daytime: ResMut<DaylightOffset>) {
    daytime.offset = options.daytime * 10. / options.day_speed;
    daytime.stop_day = options.stop_day;
    daytime.day_speed = options.day_speed;
}

fn write_settings_to_file(
    time: Res<Time>,
    options: Res<Options>,
    mut last_options: Local<Options>,
    mut last_time: Local<f32>,
) {
    // ensure the time difference is > 1 second
    let secs = time.elapsed_seconds_wrapped();
    if (*last_time - secs).abs() < 1. {
        return;
    }

    *last_time = secs;

    if options.is_not_similar(&last_options) {
        *last_options = options.clone();

        if let Err(e) = options.write_options_to_file() {
            error!("Failed to write settings to file due to: {e}");
        }
    }
}

fn update_camera_state(mut primary_camera: Query<&mut PrimaryCamera>, options: Res<Options>) {
    if PrimaryCamera::Director(0) == options.camera_state {
        if let PrimaryCamera::Director(_) = primary_camera.single() {
            return;
        }
    }

    *primary_camera.single_mut() = options.camera_state;
}

fn listen(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut picking_state: ResMut<PickingPluginsSettings>,
    key: Res<ButtonInput<KeyCode>>,
    mut menu_focused: ResMut<MenuFocused>,
    mut last_focus: Local<bool>,
    mut options: ResMut<Options>,
) {
    if key.just_pressed(KeyCode::Escape) {
        menu_focused.0 = !menu_focused.0;
    }

    if *last_focus != menu_focused.0 {
        let mut window = windows.single_mut();
        window.cursor.grab_mode = if menu_focused.0 {
            CursorGrabMode::None
        } else if cfg!(windows) {
            CursorGrabMode::Confined
        } else {
            CursorGrabMode::Locked
        };

        window.cursor.visible = menu_focused.0;
        picking_state.enable = menu_focused.0;
    }

    *last_focus = menu_focused.0;

    if menu_focused.0 {
        return;
    }

    if key.just_pressed(KeyCode::Digit1) || key.just_pressed(KeyCode::Numpad1) {
        options.camera_state = PrimaryCamera::TrackCar(1);
    } else if key.just_pressed(KeyCode::Digit2) || key.just_pressed(KeyCode::Numpad2) {
        options.camera_state = PrimaryCamera::TrackCar(2);
    } else if key.just_pressed(KeyCode::Digit3) || key.just_pressed(KeyCode::Numpad3) {
        options.camera_state = PrimaryCamera::TrackCar(3);
    } else if key.just_pressed(KeyCode::Digit4) || key.just_pressed(KeyCode::Numpad4) {
        options.camera_state = PrimaryCamera::TrackCar(4);
    } else if key.just_pressed(KeyCode::Digit5) || key.just_pressed(KeyCode::Numpad5) {
        options.camera_state = PrimaryCamera::TrackCar(5);
    } else if key.just_pressed(KeyCode::Digit6) || key.just_pressed(KeyCode::Numpad2) {
        options.camera_state = PrimaryCamera::TrackCar(6);
    } else if key.just_pressed(KeyCode::Digit7) || key.just_pressed(KeyCode::Numpad7) {
        options.camera_state = PrimaryCamera::TrackCar(7);
    } else if key.just_pressed(KeyCode::Digit8) || key.just_pressed(KeyCode::Numpad8) {
        options.camera_state = PrimaryCamera::TrackCar(8);
    } else if key.just_pressed(KeyCode::Digit9) || key.just_pressed(KeyCode::Numpad9) {
        options.camera_state = PrimaryCamera::Director(0);
    } else if key.just_pressed(KeyCode::Digit0) || key.just_pressed(KeyCode::Numpad0) {
        options.camera_state = PrimaryCamera::Spectator;
    }
}
