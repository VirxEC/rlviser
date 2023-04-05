use bevy::{
    prelude::*,
    window::{PresentMode, PrimaryWindow},
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use warbler_grass::prelude::*;

use crate::{
    camera::DaylightOffset,
    mesh::{get_grass, GrassLod},
};

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(EguiPlugin)
            .insert_resource(Options::default())
            .add_system(ui_system)
            .add_system(toggle_vsync.after(ui_system))
            .add_system(toggle_vsync)
            .add_system(update_grass.after(ui_system))
            .add_system(update_grass)
            .add_system(update_daytime.after(ui_system))
            .add_system(update_daytime);
    }
}

#[derive(Resource)]
struct Options {
    focus: bool,
    vsync: bool,
    fps: (usize, [f32; 25]),
    grass_lod: u8,
    stop_day: bool,
    daytime: f32,
    day_speed: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            focus: false,
            vsync: true,
            fps: Default::default(),
            grass_lod: 2,
            stop_day: false,
            daytime: 0.,
            day_speed: 1.,
        }
    }
}

fn ui_system(mut contexts: EguiContexts, time: Res<Time>, keys: Res<Input<KeyCode>>, mut options: ResMut<Options>) {
    if keys.just_pressed(KeyCode::I) {
        options.focus = !options.focus;
    }

    if options.focus {
        return;
    }

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
        ui.label("Press I to hide");
        ui.label(format!("FPS: {fps:.0}"));
        ui.checkbox(&mut options.vsync, "vsync");
        ui.add(egui::Slider::new(&mut options.grass_lod, 0..=3).text("Grass LOD"));
        ui.checkbox(&mut options.stop_day, "Stop day cycle");
        ui.add(egui::Slider::new(&mut options.daytime, 0.0..=150.0).text("Daytime"));
        ui.add(egui::Slider::new(&mut options.day_speed, 0.0..=10.0).text("Day speed"));
    });
}

fn toggle_vsync(options: Res<Options>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    let wanted_present_mode = if options.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };

    if windows.single().present_mode == wanted_present_mode {
        return;
    }

    windows.single_mut().present_mode = wanted_present_mode;
}

fn update_grass(options: Res<Options>, mut lod: ResMut<GrassLod>, mut query: Query<(&mut Grass, &mut Transform)>) {
    if options.grass_lod == lod.get() {
        return;
    }

    let (mut grass, mut transform) = query.single_mut();

    let (positions, height, scale) = get_grass(options.grass_lod);

    grass.positions = positions;
    grass.height = height;
    *transform = scale;

    lod.set(options.grass_lod);
}

fn update_daytime(options: Res<Options>, mut daytime: ResMut<DaylightOffset>) {
    daytime.offset = options.daytime * 10. / options.day_speed;
    daytime.stop_day = options.stop_day;
    daytime.day_speed = options.day_speed;
}
