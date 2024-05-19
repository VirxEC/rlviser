use super::{
    options::{BallCam, CalcBallRot, Extrapolation, GameOptions, GameSpeed, MenuFocused, ShowTime, UiOverlayScale},
    state_setting::StateSettingInterface,
};
use crate::{
    bytes::ToBytesExact,
    camera::{DaylightOffset, PrimaryCamera, Sun},
    renderer::{DoRendering, RenderGroups},
    spectator::SpectatorSettings,
    udp::{Connection, PausedUpdate, SpeedUpdate, UdpPacketTypes},
};
use bevy::{
    pbr::DirectionalLightShadowMap,
    prelude::*,
    time::Stopwatch,
    window::{CursorGrabMode, PrimaryWindow},
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bevy_framepace::{FramepaceSettings, Limiter};
use bevy_mod_picking::picking_core::PickingPluginsSettings;
use std::{
    fs,
    io::{self, Write},
    time::Duration,
};

#[cfg(debug_assertions)]
use crate::camera::{EntityName, HighlightedEntity};

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((EguiPlugin, GameOptions, StateSettingInterface))
            .insert_resource(Options::default_read_file())
            .insert_resource(RenderInfo::default())
            .insert_resource(UpdateRenderInfoTime::default())
            .insert_resource(PacketSendTime::default())
            .add_systems(
                Update,
                (
                    listen,
                    (read_speed_update_event, read_paused_update_event),
                    (
                        advance_time,
                        ui_system,
                        toggle_vsync,
                        toggle_ballcam,
                        toggle_show_time,
                        update_daytime,
                        #[cfg(not(feature = "ssao"))]
                        update_msaa,
                        update_ui_scale,
                        update_shadows,
                        update_sensitivity,
                        update_allow_rendering,
                        update_render_info,
                        update_extrapolation,
                        update_calc_ball_rot,
                        (
                            update_speed
                                .run_if(|options: Res<Options>, last: Res<GameSpeed>| options.game_speed != last.speed),
                            update_paused
                                .run_if(|options: Res<Options>, last: Res<GameSpeed>| options.paused != last.paused),
                        )
                            .run_if(resource_exists::<Connection>),
                    )
                        .run_if(resource_equals(MenuFocused::default())),
                    update_camera_state,
                    write_settings_to_file,
                )
                    .chain(),
            );

        #[cfg(debug_assertions)]
        app.add_systems(Update, debug_ui);
    }
}

#[derive(Resource, Default)]
struct PacketSendTime(Stopwatch);

fn advance_time(mut last_packet_send: ResMut<PacketSendTime>, time: Res<Time>) {
    last_packet_send.0.tick(time.delta());
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
    game_speed: f32,
    paused: bool,
    mouse_sensitivity: f32,
    allow_rendering: bool,
    extrapolation: bool,
    calc_ball_rot: bool,
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
            game_speed: 1.,
            paused: false,
            mouse_sensitivity: 1.,
            allow_rendering: true,
            extrapolation: false,
            calc_ball_rot: true,
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
                "game_speed" => options.game_speed = value.parse().unwrap(),
                "paused" => options.paused = value.parse().unwrap(),
                "mouse_sensitivity" => options.mouse_sensitivity = value.parse().unwrap(),
                "allow_rendering" => options.allow_rendering = value.parse().unwrap(),
                "extrapolation" => options.extrapolation = value.parse().unwrap(),
                "calc_ball_rot" => options.calc_ball_rot = value.parse().unwrap(),
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
        file.write_fmt(format_args!("game_speed={}\n", self.game_speed))?;
        file.write_fmt(format_args!("paused={}\n", self.paused))?;
        file.write_fmt(format_args!("mouse_sensitivity={}\n", self.mouse_sensitivity))?;
        file.write_fmt(format_args!("allow_rendering={}\n", self.allow_rendering))?;
        file.write_fmt(format_args!("extrapolation={}\n", self.extrapolation))?;
        file.write_fmt(format_args!("calc_ball_rot={}\n", self.calc_ball_rot))?;

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
            || self.game_speed != other.game_speed
            || self.paused != other.paused
            || self.mouse_sensitivity != other.mouse_sensitivity
            || self.allow_rendering != other.allow_rendering
            || self.extrapolation != other.extrapolation
            || self.calc_ball_rot != other.calc_ball_rot
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

#[derive(Resource, Default)]
struct UpdateRenderInfoTime(Stopwatch);

#[derive(Resource, Default)]
struct RenderInfo {
    groups: usize,
    items: usize,
}

fn update_render_info(
    renders: Res<RenderGroups>,
    mut render_info: ResMut<RenderInfo>,
    mut last_render_update: ResMut<UpdateRenderInfoTime>,
    time: Res<Time>,
) {
    last_render_update.0.tick(time.delta());

    if last_render_update.0.elapsed() < Duration::from_secs_f32(1. / 10.) {
        last_render_update.0.reset();
        return;
    }

    render_info.groups = renders.groups.len();
    render_info.items = renders.groups.values().map(Vec::len).sum();
}

fn ui_system(
    mut menu_focused: ResMut<MenuFocused>,
    mut options: ResMut<Options>,
    mut contexts: EguiContexts,
    render_info: Res<RenderInfo>,
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

            ui.horizontal(|ui| {
                ui.label("Game speed");
                ui.add(
                    egui::DragValue::new(&mut options.game_speed)
                        .clamp_range(0.1..=10.0)
                        .speed(0.02)
                        .fixed_decimals(1),
                );
                ui.checkbox(&mut options.paused, "Paused");
            });

            ui.checkbox(&mut options.extrapolation, "Packet extrapolation");
            ui.checkbox(&mut options.calc_ball_rot, "Ignore packet ball rotation");

            ui.menu_button("Open rendering manager", |ui| {
                ui.checkbox(&mut options.allow_rendering, "Allow rendering");

                ui.add_space(10.);

                ui.label(format!("Groups: {}", render_info.groups));
                ui.label(format!("Items: {}", render_info.items));
            });

            ui.add_space(10.);

            ui.horizontal(|ui| {
                ui.checkbox(&mut options.show_time, "In-game time");
                ui.checkbox(&mut options.ball_cam, "Ball cam");
            });
            ui.add(egui::Slider::new(&mut options.ui_scale, 0.4..=4.0).text("UI scale"));
            ui.label("Mouse sensitivity:");
            ui.add(egui::Slider::new(&mut options.mouse_sensitivity, 0.01..=4.0));

            ui.add_space(10.);

            ui.checkbox(&mut options.stop_day, "Stop day cycle");
            ui.add(egui::Slider::new(&mut options.daytime, 0.0..=150.0).text("Daytime"));
            ui.add(egui::Slider::new(&mut options.day_speed, 0.0..=10.0).text("Day speed"));
        });
}

fn update_allow_rendering(options: Res<Options>, mut do_rendering: ResMut<DoRendering>, mut renders: ResMut<RenderGroups>) {
    if !options.allow_rendering {
        renders.groups.clear();
    }

    do_rendering.0 = options.allow_rendering;
}

fn update_sensitivity(options: Res<Options>, mut settings: ResMut<SpectatorSettings>) {
    settings.sensitivity = SpectatorSettings::default().sensitivity * options.mouse_sensitivity;
}

fn read_speed_update_event(
    mut events: EventReader<SpeedUpdate>,
    mut options: ResMut<Options>,
    mut game_speed: ResMut<GameSpeed>,
) {
    for event in events.read() {
        options.game_speed = event.0;
        game_speed.speed = event.0;
    }
}

fn read_paused_update_event(
    mut events: EventReader<PausedUpdate>,
    mut options: ResMut<Options>,
    mut game_speed: ResMut<GameSpeed>,
) {
    for event in events.read() {
        options.paused = event.0;
        game_speed.paused = event.0;
    }
}

fn update_extrapolation(options: Res<Options>, mut extrapolation: ResMut<Extrapolation>) {
    extrapolation.0 = options.extrapolation;
}

fn update_speed(
    options: Res<Options>,
    socket: Res<Connection>,
    mut last_packet_send: ResMut<PacketSendTime>,
    time: Res<Time>,
    mut global: ResMut<GameSpeed>,
) {
    last_packet_send.0.tick(time.delta());
    if last_packet_send.0.elapsed() < Duration::from_secs_f32(1. / 15.) {
        last_packet_send.0.reset();
        return;
    }

    if let Err(e) = socket.0.send_to(&[UdpPacketTypes::Speed as u8], socket.1) {
        error!("Failed to change game speed: {e}");
    }

    if let Err(e) = socket.0.send_to(&options.game_speed.to_bytes(), socket.1) {
        error!("Failed to change game speed: {e}");
    }

    global.speed = options.game_speed;
}

fn update_paused(options: Res<Options>, socket: Res<Connection>, mut global: ResMut<GameSpeed>) {
    if let Err(e) = socket.0.send_to(&[UdpPacketTypes::Paused as u8], socket.1) {
        error!("Failed to change game speed: {e}");
    }

    if let Err(e) = socket.0.send_to(&options.paused.to_bytes(), socket.1) {
        error!("Failed to change game speed: {e}");
    }

    global.paused = options.paused;
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

fn update_calc_ball_rot(options: Res<Options>, mut calc_ball_rot: ResMut<CalcBallRot>) {
    calc_ball_rot.0 = options.calc_ball_rot;
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

fn update_ui_scale(options: Res<Options>, mut ui_scale: ResMut<UiOverlayScale>) {
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
        picking_state.is_enabled = menu_focused.0;
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
