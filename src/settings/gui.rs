use super::options::{
    BallCam, CalcBallRot, GameOptions, GameSpeed, MenuFocused, Options, PacketSmoothing, ShowTime, UiOverlayScale,
};
use crate::{
    camera::{DaylightOffset, PrimaryCamera},
    renderer::{DoRendering, RenderGroups},
    spectator::SpectatorSettings,
    udp::{Connection, LastPacketTimesElapsed, PausedUpdate, SendableUdp, SpeedUpdate},
};
use bevy::{
    light::{DirectionalLightShadowMap, SunDisk},
    picking::PickingSettings,
    prelude::*,
    time::Stopwatch,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use std::time::Duration;

use super::state_setting::StateSettingInterface;
use bevy_egui::{EguiContext, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext, egui};
use bevy_framepace::{FramepaceSettings, Limiter};

#[cfg(debug_assertions)]
use crate::camera::{EntityName, HighlightedEntity};

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((GameOptions, StateSettingInterface, EguiPlugin::default()))
            .insert_resource(RenderInfo::default())
            .insert_resource(UpdateRenderInfoTime::default())
            .insert_resource(PacketSendTime::default())
            .add_systems(
                EguiPrimaryContextPass,
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
                        update_msaa,
                        update_ui_scale,
                        update_shadows,
                        update_sensitivity,
                        update_allow_rendering,
                        update_render_info,
                        update_packet_smoothing,
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
        app.add_systems(EguiPrimaryContextPass, debug_ui);
    }
}

#[derive(Resource, Default)]
struct PacketSendTime(Stopwatch);

fn advance_time(mut last_packet_send: ResMut<PacketSendTime>, time: Res<Time>) {
    last_packet_send.0.tick(time.delta());
}

#[cfg(debug_assertions)]
fn debug_ui(
    mut contexts: Single<&mut EguiContext, With<PrimaryEguiContext>>,
    heq: Query<(&Transform, &EntityName), With<HighlightedEntity>>,
    cam_pos: Query<&Transform, With<PrimaryCamera>>,
) {
    let ctx = contexts.get_mut();
    let camera_pos = cam_pos.single().unwrap().translation;

    let (he_pos, highlighted_entity_name) = heq.single().map_or_else(
        |_| (Vec3::default(), Box::from("None")),
        |(transform, he)| (transform.translation, he.name.clone()),
    );

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
    mut context: Single<&mut EguiContext, With<PrimaryEguiContext>>,
    render_info: Res<RenderInfo>,
    time: Res<Time>,
) {
    const MSAA_NAMES: [&str; 4] = ["Off", "2x", "4x", "8x"];
    const SHADOW_NAMES: [&str; 4] = ["Off", "0.5x", "1x", "2x"];
    const SMOOTHING_NAMES: [&str; 3] = ["None", "Interpolate", "Extrapolate"];

    let ctx = context.get_mut();

    let dt = time.delta_secs();
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
        .open(&mut menu_focused)
        .show(ctx, |ui| {
            ui.label(format!("FPS: {fps:.0}"));

            ui.collapsing("Graphics", |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut options.vsync, "vsync");
                    ui.checkbox(&mut options.uncap_fps, "Uncap FPS");
                    ui.add(egui::DragValue::new(&mut options.fps_limit).speed(5.).range(30..=600));
                });

                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("Shadows").width(50.).show_index(
                        ui,
                        &mut options.shadows,
                        SHADOW_NAMES.len(),
                        |i| SHADOW_NAMES[i],
                    );
                    egui::ComboBox::from_label("MSAA")
                        .width(40.)
                        .show_index(ui, &mut options.msaa, MSAA_NAMES.len(), |i| MSAA_NAMES[i]);
                });

                egui::ComboBox::from_label("Packet smoothing").width(100.).show_index(
                    ui,
                    &mut options.packet_smoothing as &mut usize,
                    SMOOTHING_NAMES.len(),
                    |i| SMOOTHING_NAMES[i],
                );
                ui.checkbox(&mut options.calc_ball_rot, "Ignore packet ball rotation");
            });

            egui::CollapsingHeader::new("World settings")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Game speed");
                        ui.add(
                            egui::DragValue::new(&mut options.game_speed)
                                .range(0.01..=10.0)
                                .speed(0.02)
                                .fixed_decimals(2),
                        );
                        ui.checkbox(&mut options.paused, "Paused");
                    });

                    ui.add_space(15.);

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut options.show_time, "In-game time");
                        ui.checkbox(&mut options.ball_cam, "Ball cam");
                    });
                    ui.add(egui::Slider::new(&mut options.ui_scale, 0.4..=4.0).text("UI scale"));
                    ui.label("Mouse sensitivity:");
                    ui.add(egui::Slider::new(&mut options.mouse_sensitivity, 0.01..=4.0));

                    ui.add_space(15.);

                    ui.checkbox(&mut options.stop_day, "Stop day cycle");
                    ui.add(egui::Slider::new(&mut options.daytime, 0.0..=150.0).text("Daytime"));
                    ui.add(egui::Slider::new(&mut options.day_speed, 0.0..=10.0).text("Day speed"));
                });

            ui.collapsing("Rendering manager", |ui| {
                ui.checkbox(&mut options.allow_rendering, "Allow rendering");

                ui.add_space(10.);

                ui.label(format!("Groups: {}", render_info.groups));
                ui.label(format!("Items: {}", render_info.items));
            });
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
    mut events: MessageReader<SpeedUpdate>,
    mut options: ResMut<Options>,
    mut game_speed: ResMut<GameSpeed>,
) {
    for event in events.read() {
        options.game_speed = event.0;
        game_speed.speed = event.0;
    }
}

fn read_paused_update_event(
    mut events: MessageReader<PausedUpdate>,
    mut options: ResMut<Options>,
    mut game_speed: ResMut<GameSpeed>,
) {
    for event in events.read() {
        options.paused = event.0;
        game_speed.paused = event.0;
    }
}

fn update_packet_smoothing(options: Res<Options>, mut packet_smoothing: ResMut<PacketSmoothing>) {
    *packet_smoothing = PacketSmoothing::from_usize(options.packet_smoothing);
}

fn update_speed(
    options: Res<Options>,
    socket: Res<Connection>,
    mut last_packet_send: ResMut<PacketSendTime>,
    mut last_packet_times: ResMut<LastPacketTimesElapsed>,
    time: Res<Time>,
    mut global: ResMut<GameSpeed>,
) {
    last_packet_send.0.tick(time.delta());
    if last_packet_send.0.elapsed() < Duration::from_secs_f32(1. / 15.) {
        last_packet_send.0.reset();
        return;
    }

    last_packet_times.reset();
    socket.send(SendableUdp::Speed(options.game_speed)).unwrap();
    global.speed = options.game_speed;
}

fn update_paused(options: Res<Options>, socket: Res<Connection>, mut global: ResMut<GameSpeed>) {
    socket.send(SendableUdp::Paused(options.paused)).unwrap();
    global.paused = options.paused;
}

fn update_shadows(
    options: Res<Options>,
    mut query: Query<&mut DirectionalLight, With<SunDisk>>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
) {
    query.single_mut().unwrap().shadows_enabled = options.shadows != 0;
    shadow_map.size = 2048 * 2usize.pow(options.shadows.max(1) as u32 - 1);
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

fn update_msaa(options: Res<Options>, mut msaa_query: Query<&mut Msaa>) {
    const MSAA_SAMPLES: [u32; 4] = [1, 2, 4, 8];

    for mut msaa in &mut msaa_query {
        if MSAA_SAMPLES[options.msaa] == msaa.samples() {
            continue;
        }

        *msaa = match options.msaa {
            0 => Msaa::Off,
            1 => Msaa::Sample2,
            2 => Msaa::Sample4,
            3 => Msaa::Sample8,
            _ => unreachable!(),
        };
    }
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
    let secs = time.elapsed_secs_wrapped();
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
    if PrimaryCamera::Director(0) == options.camera_state
        && let PrimaryCamera::Director(_) = primary_camera.single().unwrap()
    {
        return;
    }

    *primary_camera.single_mut().unwrap() = options.camera_state;
}

fn listen(
    mut cursor_options: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut picking_state: ResMut<PickingSettings>,
    key: Res<ButtonInput<KeyCode>>,
    mut menu_focused: ResMut<MenuFocused>,
    mut last_focus: Local<bool>,
    mut options: ResMut<Options>,
) {
    if key.just_pressed(KeyCode::Escape) {
        menu_focused.0 = !menu_focused.0;
    }

    if *last_focus != menu_focused.0 {
        let mut cursor_options = cursor_options.single_mut().unwrap();
        cursor_options.grab_mode = if menu_focused.0 {
            CursorGrabMode::None
        } else if cfg!(windows) {
            CursorGrabMode::Confined
        } else {
            CursorGrabMode::Locked
        };

        cursor_options.visible = menu_focused.0;
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
