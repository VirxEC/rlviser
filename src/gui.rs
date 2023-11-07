use crate::camera::{DaylightOffset, PrimaryCamera, Sun};
use bevy::{
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

#[derive(Resource, PartialEq, Eq)]
struct MenuFocused(pub bool);

impl Default for MenuFocused {
    #[inline]
    fn default() -> Self {
        Self(true)
    }
}

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
    key: Res<Input<KeyCode>>,
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

    if key.just_pressed(KeyCode::Key1) || key.just_pressed(KeyCode::Numpad1) {
        options.camera_state = PrimaryCamera::TrackCar(1);
    } else if key.just_pressed(KeyCode::Key2) || key.just_pressed(KeyCode::Numpad2) {
        options.camera_state = PrimaryCamera::TrackCar(2);
    } else if key.just_pressed(KeyCode::Key3) || key.just_pressed(KeyCode::Numpad3) {
        options.camera_state = PrimaryCamera::TrackCar(3);
    } else if key.just_pressed(KeyCode::Key4) || key.just_pressed(KeyCode::Numpad4) {
        options.camera_state = PrimaryCamera::TrackCar(4);
    } else if key.just_pressed(KeyCode::Key5) || key.just_pressed(KeyCode::Numpad5) {
        options.camera_state = PrimaryCamera::TrackCar(5);
    } else if key.just_pressed(KeyCode::Key6) || key.just_pressed(KeyCode::Numpad2) {
        options.camera_state = PrimaryCamera::TrackCar(6);
    } else if key.just_pressed(KeyCode::Key7) || key.just_pressed(KeyCode::Numpad7) {
        options.camera_state = PrimaryCamera::TrackCar(7);
    } else if key.just_pressed(KeyCode::Key8) || key.just_pressed(KeyCode::Numpad8) {
        options.camera_state = PrimaryCamera::TrackCar(8);
    } else if key.just_pressed(KeyCode::Key9) || key.just_pressed(KeyCode::Numpad9) {
        options.camera_state = PrimaryCamera::Director(0);
    } else if key.just_pressed(KeyCode::Key0) || key.just_pressed(KeyCode::Numpad0) {
        options.camera_state = PrimaryCamera::Spectator;
    }
}
