mod assets;
mod bytes;
mod camera;
mod gui;
mod mesh;
mod rocketsim;
mod udp;

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum LoadState {
    #[default]
    Assets,
    Field,
    FieldExtra,
    Connect,
    None,
}

#[derive(Resource)]
pub struct ServerPort {
    primary_port: u16,
    secondary_port: u16,
}

fn main() {
    // read the first argument and treat it as the port to connect to (u16)
    let primary_port = std::env::args().nth(1).and_then(|s| s.parse::<u16>().ok()).unwrap_or(34254);
    // read the second argument and treat it as the port to bind the UDP socket to (u16)
    let secondary_port = std::env::args().nth(1).and_then(|s| s.parse::<u16>().ok()).unwrap_or(45243);

    assets::uncook().unwrap();

    App::new()
        .add_state::<LoadState>()
        .insert_resource(ServerPort { primary_port, secondary_port })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RLViser-rs".into(),
                ..default()
            }),
            ..default()
        }))
        .add_asset_loader(assets::PskxLoader)
        .add_plugin(bevy::diagnostic::LogDiagnosticsPlugin::default())
        .add_loading_state(LoadingState::new(LoadState::Assets).continue_to_state(LoadState::Field))
        .add_collection_to_loading_state::<_, assets::BallAssets>(LoadState::Assets)
        .add_collection_to_loading_state::<_, assets::TiledPatterns>(LoadState::Assets)
        .add_collection_to_loading_state::<_, assets::Details>(LoadState::Assets)
        .add_collection_to_loading_state::<_, assets::ParkStadium>(LoadState::Assets)
        .add_collection_to_loading_state::<_, assets::FutureStadium>(LoadState::Assets)
        .add_plugin(camera::CameraPlugin)
        .add_plugin(gui::DebugOverlayPlugin)
        .add_plugin(mesh::FieldLoaderPlugin)
        .add_plugin(udp::RocketSimPlugin)
        .run();
}
