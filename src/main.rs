mod camera;
mod rocketsim;

use camera::CameraPlugin;
use rocketsim::RocketSimPlugin;

use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_plugin(RocketSimPlugin)
        .add_plugin(CameraPlugin)
        .run();
}
