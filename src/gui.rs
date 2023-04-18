use bevy::{
    prelude::*,
    window::{PresentMode, PrimaryWindow},
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::camera::DaylightOffset;

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
            .add_system(update_msaa);
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
        ui.checkbox(&mut options.stop_day, "Stop day cycle");
        ui.add(egui::Slider::new(&mut options.daytime, 0.0..=150.0).text("Daytime"));
        ui.add(egui::Slider::new(&mut options.day_speed, 0.0..=10.0).text("Day speed"));
        ui.add(egui::Slider::new(&mut options.msaa, 0..=3).text("MSAA"));
    });
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
