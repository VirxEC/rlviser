mod camera;
mod gui;
mod mesh;
mod rocketsim;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RLViser-rs".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugin(bevy::diagnostic::LogDiagnosticsPlugin::default())
        .add_plugin(rocketsim::RocketSimPlugin)
        .add_plugin(camera::CameraPlugin)
        .add_plugin(gui::DebugOverlayPlugin)
        .add_plugin(mesh::FieldLoaderPlugin)
        .run();
}
