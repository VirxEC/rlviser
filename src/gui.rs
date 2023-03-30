use bevy::{
    prelude::*,
    window::{PresentMode, PrimaryWindow},
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(EguiPlugin)
            .insert_resource(Options::default())
            .add_system(ui_system)
            .add_system(toggle_vsync.after(ui_system))
            .add_system(toggle_vsync);
    }
}

#[derive(Resource)]
struct Options {
    focus: bool,
    vsync: bool,
    fps: (usize, [f32; 25]),
}

impl Default for Options {
    fn default() -> Self {
        Self {
            focus: false,
            vsync: true,
            fps: Default::default(),
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
    });
}

fn toggle_vsync(options: Res<Options>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    let wanted_present_mode = if options.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };

    if windows.single().present_mode == wanted_present_mode {
        return;
    }

    windows.single_mut().present_mode = wanted_present_mode;
}
