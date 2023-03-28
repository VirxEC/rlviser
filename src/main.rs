mod camera;
mod mesh;
mod rocketsim;

use camera::CameraPlugin;
use mesh::FieldLoaderPlugin;
use rocketsim::RocketSimPlugin;

use bevy::{diagnostic::LogDiagnosticsPlugin, prelude::*};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RLViser-rs".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(RocketSimPlugin)
        .add_plugin(CameraPlugin)
        .add_plugin(FieldLoaderPlugin)
        .run();
}
