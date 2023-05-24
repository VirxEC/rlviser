use std::{
    fs,
    io::{self, Write},
};

use bevy::{
    prelude::*,
    render::camera::CameraProjection,
    window::{CursorGrabMode, PresentMode, PrimaryWindow},
};
use bevy_atmosphere::prelude::AtmosphereCamera;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bevy_mod_picking::{PickingCameraBundle, PickingPluginsState};

use crate::{
    camera::{DaylightOffset, EntityName, HighlightedEntity, PrimaryCamera},
    spectator::Spectator,
};

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(EguiPlugin)
            .insert_resource(Msaa::default())
            .insert_resource(Options::read_from_file().unwrap_or_else(|_| Options::create_file_from_defualt()))
            .add_system(listen)
            .add_system(ui_system)
            .add_system(toggle_vsync.after(ui_system))
            .add_system(toggle_vsync)
            .add_system(update_daytime.after(ui_system))
            .add_system(update_daytime)
            .add_system(update_msaa.after(ui_system))
            .add_system(update_msaa)
            .add_system(update_draw_distance.after(ui_system))
            .add_system(update_draw_distance)
            .add_system(write_settings_to_file.after(ui_system))
            .add_system(write_settings_to_file);
    }
}

#[derive(Clone, Resource)]
struct Options {
    focus: bool,
    vsync: bool,
    fps: (usize, [f32; 25]),
    stop_day: bool,
    daytime: f32,
    day_speed: f32,
    msaa: u8,
    draw_distance: u8,
}

impl Default for Options {
    #[inline]
    fn default() -> Self {
        Self {
            focus: false,
            vsync: true,
            fps: Default::default(),
            stop_day: false,
            daytime: 0.,
            day_speed: 1.,
            msaa: 2,
            draw_distance: 3,
        }
    }
}

impl Options {
    const FILE_NAME: &str = "settings.txt";

    fn read_from_file() -> io::Result<Self> {
        let mut options = Options::default();

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
                "stop_day" => options.stop_day = value.parse().unwrap(),
                "daytime" => options.daytime = value.parse().unwrap(),
                "day_speed" => options.day_speed = value.parse().unwrap(),
                "msaa" => options.msaa = value.parse().unwrap(),
                _ => println!("Unknown key {key} with value {value}"),
            }
        }

        Ok(options)
    }

    fn create_file_from_defualt() -> Self {
        let options = Options::default();

        if let Err(e) = options.write_options_to_file() {
            println!("Failed to create {} due to: {e}", Self::FILE_NAME);
        }

        options
    }

    fn write_options_to_file(&self) -> io::Result<()> {
        let mut file = fs::File::create(Self::FILE_NAME)?;

        file.write_fmt(format_args!("vsync={}\n", self.vsync))?;
        file.write_fmt(format_args!("stop_day={}\n", self.stop_day))?;
        file.write_fmt(format_args!("daytime={}\n", self.daytime))?;
        file.write_fmt(format_args!("day_speed={}\n", self.day_speed))?;
        file.write_fmt(format_args!("msaa={}\n", self.msaa))?;

        Ok(())
    }

    #[inline]
    fn is_not_similar(&self, other: &Options) -> bool {
        self.vsync != other.vsync || self.stop_day != other.stop_day || self.daytime != other.daytime || self.day_speed != other.day_speed || self.msaa != other.msaa
    }
}

#[allow(clippy::too_many_arguments)]
fn ui_system(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut picking_state: ResMut<PickingPluginsState>,
    mut options: ResMut<Options>,
    mut contexts: EguiContexts,
    heq: Query<(&Transform, &EntityName), With<HighlightedEntity>>,
    cam_pos: Query<&Transform, With<PrimaryCamera>>,
    keys: Res<Input<KeyCode>>,
    time: Res<Time>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        options.focus = !options.focus;

        let mut window = windows.single_mut();
        window.cursor.grab_mode = match options.focus {
            true => {
                if cfg!(windows) {
                    CursorGrabMode::Confined
                } else {
                    CursorGrabMode::Locked
                }
            }
            false => CursorGrabMode::None,
        };
        window.cursor.visible = !options.focus;
        picking_state.enable_picking = !options.focus;
        picking_state.enable_interacting = !options.focus;
        picking_state.enable_highlighting = !options.focus;
    }

    if options.focus {
        return;
    }

    let (he_pos, highlighted_entity_name) = heq
        .get_single()
        .map(|(transform, he)| (transform.translation, he.name.clone()))
        .unwrap_or((Vec3::default(), String::from("None")));

    let ctx = contexts.ctx_mut();

    let dt = time.raw_delta_seconds_f64();
    if dt == 0.0 {
        return;
    }

    let (i, history) = &mut options.fps;

    history[*i] = dt as f32;
    *i += 1;
    *i %= history.len();

    let avg_dt = history.iter().sum::<f32>() / history.len() as f32;
    let fps = 1. / avg_dt;

    let camera_pos = cam_pos.single().translation;

    egui::SidePanel::left("left_panel").show(ctx, |ui| {
        ui.label("Press Esc to close menu");
        ui.label(format!("FPS: {fps:.0}"));
        ui.checkbox(&mut options.vsync, "vsync");
        ui.checkbox(&mut options.stop_day, "Stop day cycle");
        ui.add(egui::Slider::new(&mut options.daytime, 0.0..=150.0).text("Daytime"));
        ui.add(egui::Slider::new(&mut options.day_speed, 0.0..=10.0).text("Day speed"));
        ui.add(egui::Slider::new(&mut options.msaa, 0..=3).text("MSAA"));
        // ui.add(egui::Slider::new(&mut options.draw_distance, 0..=4).text("Draw distance"));
        ui.label(format!("Primary camera position: [{:.0}, {:.0}, {:.0}]", camera_pos.x, camera_pos.y, camera_pos.z));
        ui.label(format!("HE position: [{:.0}, {:.0}, {:.0}]", he_pos.x, he_pos.y, he_pos.z));
        ui.label(format!("Highlighted entity: {highlighted_entity_name}"));
    });
}

fn update_draw_distance(options: Res<Options>, mut commands: Commands, query: Query<(&PrimaryCamera, &Projection, &Transform, Entity)>) {
    let draw_distance = match options.draw_distance {
        0 => 15000.,
        1 => 50000.,
        2 => 200000.,
        3 => 500000.,
        4 => 2000000.,
        _ => unreachable!(),
    };

    let (primary_camera, projection, transform, entity) = query.single();

    if projection.far() == draw_distance {
        return;
    }

    info!("Setting draw distance to {draw_distance}");
    commands.entity(entity).despawn_recursive();

    commands
        .spawn((
            *primary_camera,
            Camera3dBundle {
                projection: PerspectiveProjection { far: draw_distance, ..default() }.into(),
                transform: *transform,
                ..default()
            },
        ))
        .insert((AtmosphereCamera::default(), Spectator, PickingCameraBundle::default()));
}

fn toggle_vsync(options: Res<Options>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    let wanted_present_mode = if options.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };

    if windows.single().present_mode == wanted_present_mode {
        return;
    }

    windows.single_mut().present_mode = wanted_present_mode;
}

fn update_msaa(options: Res<Options>, mut msaa: ResMut<Msaa>) {
    if options.msaa == msaa.samples() as u8 {
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

fn update_daytime(options: Res<Options>, mut daytime: ResMut<DaylightOffset>) {
    daytime.offset = options.daytime * 10. / options.day_speed;
    daytime.stop_day = options.stop_day;
    daytime.day_speed = options.day_speed;
}

fn write_settings_to_file(time: Res<Time>, options: Res<Options>, mut last_options: Local<Options>, mut last_time: Local<f32>) {
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

fn listen(key: Res<Input<KeyCode>>, mut primary_camera: Query<&mut PrimaryCamera>) {
    let mut state = primary_camera.single_mut();

    if key.just_pressed(KeyCode::Key1) || key.just_pressed(KeyCode::Numpad1) {
        *state = PrimaryCamera::TrackCar(1);
    } else if key.just_pressed(KeyCode::Key2) || key.just_pressed(KeyCode::Numpad2) {
        *state = PrimaryCamera::TrackCar(2);
    } else if key.just_pressed(KeyCode::Key3) || key.just_pressed(KeyCode::Numpad3) {
        *state = PrimaryCamera::TrackCar(3);
    } else if key.just_pressed(KeyCode::Key4) || key.just_pressed(KeyCode::Numpad4) {
        *state = PrimaryCamera::TrackCar(4);
    } else if key.just_pressed(KeyCode::Key5) || key.just_pressed(KeyCode::Numpad5) {
        *state = PrimaryCamera::TrackCar(5);
    } else if key.just_pressed(KeyCode::Key6) || key.just_pressed(KeyCode::Numpad2) {
        *state = PrimaryCamera::TrackCar(6);
    } else if key.just_pressed(KeyCode::Key0) || key.just_pressed(KeyCode::Numpad0) {
        *state = PrimaryCamera::Spectator;
    }
}
