mod camera;
mod gui;
mod mesh;
mod udp;
// mod rocketsim;

use bevy::prelude::*;

#[derive(Resource)]
pub struct ServerPort {
    primary_port: u16,
    secondary_port: u16,
}

fn main() {
    // read the first argument and treat is as the port to connect to (u16)
    let primary_port = std::env::args().nth(1).and_then(|s| s.parse::<u16>().ok()).unwrap_or(34254);
    let secondary_port = std::env::args().nth(1).and_then(|s| s.parse::<u16>().ok()).unwrap_or(45243);

    App::new()
        .insert_resource(ServerPort { primary_port, secondary_port })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RLViser-rs".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugin(bevy::diagnostic::LogDiagnosticsPlugin::default())
        // .add_plugin(rocketsim::RocketSimPlugin)
        .add_plugin(udp::RocketSimPlugin)
        .add_plugin(camera::CameraPlugin)
        .add_plugin(gui::DebugOverlayPlugin)
        .add_plugin(mesh::FieldLoaderPlugin)
        .run();
}
