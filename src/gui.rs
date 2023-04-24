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
            .insert_resource(Options::default())
            .add_system(ui_system)
            .add_system(toggle_vsync.after(ui_system))
            .add_system(toggle_vsync)
            .add_system(update_daytime.after(ui_system))
            .add_system(update_daytime)
            .add_system(update_msaa.after(ui_system))
            .add_system(update_msaa)
            .add_system(update_draw_distance.after(ui_system))
            .add_system(update_draw_distance);
    }
}

#[derive(Resource)]
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

#[allow(clippy::too_many_arguments)]
fn ui_system(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut picking_state: ResMut<PickingPluginsState>,
    mut options: ResMut<Options>,
    mut contexts: EguiContexts,
    heq: Query<&EntityName, With<HighlightedEntity>>,
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

    let highlighted_entity_name = heq.get_single().map(|he| he.name.clone()).unwrap_or(String::from("None"));

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

    egui::SidePanel::left("left_panel").show(ctx, |ui| {
        ui.label("Press Esc to close menu");
        ui.label(format!("FPS: {fps:.0}"));
        ui.checkbox(&mut options.vsync, "vsync");
        ui.checkbox(&mut options.stop_day, "Stop day cycle");
        ui.add(egui::Slider::new(&mut options.daytime, 0.0..=150.0).text("Daytime"));
        ui.add(egui::Slider::new(&mut options.day_speed, 0.0..=10.0).text("Day speed"));
        ui.add(egui::Slider::new(&mut options.msaa, 0..=3).text("MSAA"));
        ui.add(egui::Slider::new(&mut options.draw_distance, 0..=4).text("Draw distance"));
        ui.label(format!("Highlighted entity: {highlighted_entity_name}"));
    });
}

fn update_draw_distance(options: Res<Options>, mut commands: Commands, query: Query<(&Projection, &Transform, Entity), With<PrimaryCamera>>) {
    let draw_distance = match options.draw_distance {
        0 => 15000.,
        1 => 50000.,
        2 => 200000.,
        3 => 500000.,
        4 => 2000000.,
        _ => unreachable!(),
    };

    let (projection, transform, entity) = query.iter().next().unwrap();

    if projection.far() == draw_distance {
        return;
    }

    println!("Setting draw distance to {draw_distance}");
    commands.entity(entity).remove::<(PrimaryCamera, Camera3dBundle, AtmosphereCamera, Spectator, PickingCameraBundle)>();
    commands.entity(entity).despawn();

    commands.spawn((
        PrimaryCamera,
        Camera3dBundle {
            projection: PerspectiveProjection { far: draw_distance, ..default() }.into(),
            transform: *transform,
            ..default()
        },
    )).insert((AtmosphereCamera::default(), Spectator, PickingCameraBundle::default()));
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
