#![allow(clippy::too_many_arguments, clippy::needless_pass_by_value, clippy::module_name_repetitions)]

mod assets;
mod bytes;
mod camera;
mod gui;
mod mesh;
mod morton;
mod rocketsim;
mod spectator;
mod udp;

use bevy::{
    diagnostic::LogDiagnosticsPlugin,
    prelude::*,
    render::texture::{ImageAddressMode, ImageSamplerDescriptor},
    window::PresentMode,
};
use std::env;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum LoadState {
    #[default]
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
    let mut args = env::args();

    // read the first argument and treat it as the port to connect to (u16)
    let primary_port = args.nth(1).and_then(|s| s.parse::<u16>().ok()).unwrap_or(34254);
    // read the second argument and treat it as the port to bind the UDP socket to (u16)
    let secondary_port = args.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or(45243);

    assets::uncook().unwrap();

    App::new()
        .init_state::<LoadState>()
        .insert_resource(ServerPort {
            primary_port,
            secondary_port,
        })
        .add_plugins((DefaultPlugins
            .set(ImagePlugin {
                default_sampler: ImageSamplerDescriptor {
                    address_mode_u: ImageAddressMode::Repeat,
                    address_mode_v: ImageAddressMode::Repeat,
                    address_mode_w: ImageAddressMode::Repeat,
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
            }),))
        .add_plugins((
            LogDiagnosticsPlugin {
                debug: cfg!(debug_assertions),
                ..default()
            },
            camera::CameraPlugin,
            gui::DebugOverlayPlugin,
            mesh::FieldLoaderPlugin,
            udp::RocketSimPlugin,
            assets::AssetsLoaderPlugin,
        ))
        .run();
}
