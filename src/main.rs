#![allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    clippy::module_name_repetitions,
    clippy::significant_drop_tightening,
    clippy::large_enum_variant
)]

mod assets;
mod camera;
mod flat;
mod mesh;
mod renderer;
mod settings;
mod spectator;
mod udp;

use bevy::{
    image::{ImageAddressMode, ImageSamplerDescriptor},
    log::LogPlugin,
    prelude::*,
    window::PresentMode,
};
use settings::{cache_handler, gui};
use std::env;
use tracing::Level;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum GameLoadState {
    #[default]
    Cache,
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

    #[cfg(debug_assertions)]
    assets::umodel::uncook().unwrap();

    App::new()
        .insert_resource(ServerPort {
            primary_port,
            secondary_port,
        })
        .add_plugins((
            DefaultPlugins
                .set(TaskPoolPlugin {
                    task_pool_options: TaskPoolOptions::with_num_threads(if cfg!(feature = "threaded") { 3 } else { 1 }),
                })
                .set(LogPlugin {
                    level: if cfg!(debug_assertions) { Level::INFO } else { Level::ERROR },
                    filter: if cfg!(debug_assertions) {
                        String::from("wgpu=error,naga=warn")
                    } else {
                        String::new()
                    },
                    ..Default::default()
                })
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
                }),
            cache_handler::CachePlugin,
            camera::CameraPlugin,
            gui::DebugOverlayPlugin,
            mesh::FieldLoaderPlugin,
            udp::RocketSimPlugin,
            assets::AssetsLoaderPlugin,
        ))
        .init_state::<GameLoadState>()
        .run();
}
