#![warn(clippy::all)]

mod assets;
mod bytes;
mod camera;
mod gui;
mod mesh;
mod rocketsim;
mod spectator;
mod udp;

use bevy::{
    prelude::*,
    render::render_resource::{AddressMode, SamplerDescriptor},
    window::PresentMode,
};
use bevy_asset_loader::prelude::*;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum LoadState {
    #[default]
    Assets,
    Connect,
    FieldExtra,
    Despawn,
    Field,
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
    let secondary_port = std::env::args().nth(2).and_then(|s| s.parse::<u16>().ok()).unwrap_or(45243);

    assets::uncook().unwrap();

    App::new()
        .add_state::<LoadState>()
        .insert_resource(ServerPort {
            primary_port,
            secondary_port,
        })
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin {
                    default_sampler: SamplerDescriptor {
                        address_mode_u: AddressMode::Repeat,
                        address_mode_v: AddressMode::Repeat,
                        address_mode_w: AddressMode::Repeat,
                        ..default()
                    },
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "RLViser-rs".into(),
                        present_mode: PresentMode::AutoNoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_asset_loader(assets::PskxLoader)
        .add_plugins((
            bevy::diagnostic::LogDiagnosticsPlugin::default(),
            camera::CameraPlugin,
            gui::DebugOverlayPlugin,
            mesh::FieldLoaderPlugin,
            udp::RocketSimPlugin,
        ))
        .add_loading_state(LoadingState::new(LoadState::Assets).continue_to_state(LoadState::Connect))
        .add_collection_to_loading_state::<_, assets::BallAssets>(LoadState::Assets)
        .add_collection_to_loading_state::<_, assets::BoostPickupGlows>(LoadState::Assets)
        .run();
}
