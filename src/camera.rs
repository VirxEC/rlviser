use std::{f32::consts::PI, time::Duration};

#[cfg(feature = "ssao")]
use bevy::{
    core_pipeline::experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
    pbr::ScreenSpaceAmbientOcclusionBundle,
};

use bevy::{core_pipeline::clear_color::ClearColorConfig, prelude::*};
use bevy_atmosphere::prelude::*;
use bevy_framepace::{FramepacePlugin, FramepaceSettings};
use bevy_mod_picking::prelude::*;
use bevy_vector_shapes::prelude::*;
use serde::{Deserialize, Serialize};

use crate::spectator::*;

#[derive(Component)]
struct Sun;

#[derive(Resource)]
struct CycleTimer(Timer);

#[derive(Component)]
pub struct MenuCamera;

#[derive(Component)]
pub struct BoostAmount;

#[derive(Component)]
pub struct TimeDisplay;

#[derive(Component, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimaryCamera {
    #[default]
    Spectator,
    Director(u32),
    TrackCar(u32),
}

pub const BOOST_INDICATOR_POS: Vec2 = Vec2::new(150., 150.);
pub const BOOST_INDICATOR_FONT_SIZE: f32 = 60.0;
pub const TIME_DISPLAY_POS: Vec2 = Vec2::new(0., 60.);

fn setup(mut commands: Commands) {
    // lights in the goals
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            range: 10000.,
            radius: 100.,
            intensity: 100_000.,
            ..default()
        },
        transform: Transform::from_xyz(0., 300., 5500.),
        ..default()
    });

    commands.spawn(PointLightBundle {
        point_light: PointLight {
            range: 10000.,
            radius: 100.,
            intensity: 100_000.,
            ..default()
        },
        transform: Transform::from_xyz(0., 300., -5500.),
        ..default()
    });

    commands.insert_resource(AmbientLight {
        brightness: 0.3,
        ..default()
    });

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                shadows_enabled: cfg!(feature = "ssao"),
                ..default()
            },
            ..default()
        },
        Sun,
    ));

    #[allow(unused_variables, unused_mut)]
    let mut camera_spawn = commands.spawn((
        PrimaryCamera::default(),
        Camera3dBundle {
            projection: PerspectiveProjection {
                near: 5.,
                far: 500_000.,
                fov: PI / 3.,
                ..default()
            }
            .into(),
            transform: Transform::from_translation(Vec3::new(-3000., 1000., 0.)).looking_to(Vec3::X, Vec3::Y),
            camera: Camera { hdr: true, ..default() },
            ..default()
        },
        AtmosphereCamera::default(),
        RaycastPickCamera::default(),
        Spectator,
    ));
    #[cfg(feature = "ssao")]
    camera_spawn
        .insert(ScreenSpaceAmbientOcclusionBundle::default())
        .insert(TemporalAntiAliasBundle::default());

    commands.spawn((
        MenuCamera,
        Camera2dBundle {
            camera_2d: Camera2d {
                clear_color: ClearColorConfig::None,
            },
            camera: Camera {
                order: 1,
                hdr: true,
                ..default()
            },
            transform: Transform::from_translation(Vec3::Z),
            ..default()
        },
    ));

    commands.spawn((
        TextBundle::from_section(
            "",
            TextStyle {
                font_size: BOOST_INDICATOR_FONT_SIZE,
                color: Color::SILVER,
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            right: Val::Px(BOOST_INDICATOR_POS.x - 25.),
            bottom: Val::Px(BOOST_INDICATOR_POS.y),
            ..default()
        }),
        BoostAmount,
    ));

    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                TextBundle::from_section(
                    "00m:00s",
                    TextStyle {
                        font_size: 40.0,
                        color: Color::DARK_GRAY,
                        ..default()
                    },
                )
                .with_style(Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(TIME_DISPLAY_POS.x),
                    ..default()
                }),
                TimeDisplay,
            ));
        });
}

#[derive(Resource, Default)]
pub struct DaylightOffset {
    pub offset: f32,
    pub stop_day: bool,
    pub day_speed: f32,
}

fn daylight_cycle(
    mut atmosphere: AtmosphereMut<Nishita>,
    mut query: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    mut timer: ResMut<CycleTimer>,
    offset: Res<DaylightOffset>,
    time: Res<Time>,
) {
    timer.0.tick(time.delta());

    if timer.0.finished() {
        let secs = if offset.stop_day { 0. } else { time.elapsed_seconds_wrapped() };
        let t = (offset.offset + secs) / (200. / offset.day_speed);

        let sun_position = Vec3::new(-t.cos(), t.sin(), 0.);
        atmosphere.sun_position = sun_position;

        if let Some((mut light_trans, mut directional)) = query.single_mut().into() {
            light_trans.translation = sun_position * 10_000_000.;
            light_trans.look_at(Vec3::ZERO, Vec3::Y);
            directional.illuminance = t.sin().max(0.0).powi(2) * 50000.;
        }
    }
}

#[cfg(debug_assertions)]
#[derive(Component)]
pub struct EntityName {
    pub name: Box<str>,
}

#[cfg(debug_assertions)]
impl EntityName {
    #[inline]
    pub const fn new(name: Box<str>) -> Self {
        Self { name }
    }
}

#[cfg(debug_assertions)]
impl From<&str> for EntityName {
    #[inline]
    fn from(name: &str) -> Self {
        Self::new(name.into())
    }
}

#[cfg(debug_assertions)]
impl From<String> for EntityName {
    #[inline]
    fn from(name: String) -> Self {
        Self::new(name.into())
    }
}

#[derive(Component, Clone, Copy)]
pub struct HighlightedEntity;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpectatorSettings {
            base_speed: 2500.,
            alt_speed: 750.,
            sensitivity: 0.003,
            ..default()
        })
        .insert_resource(FramepaceSettings {
            limiter: bevy_framepace::Limiter::from_framerate(60.),
        })
        .insert_resource(AtmosphereModel::default())
        .insert_resource(CycleTimer(Timer::new(
            Duration::from_secs_f32(1. / 60.),
            TimerMode::Repeating,
        )))
        .insert_resource(DaylightOffset::default())
        .add_plugins((
            FramepacePlugin,
            SpectatorPlugin,
            DefaultPickingPlugins,
            AtmospherePlugin,
            Shape2dPlugin::default(),
            #[cfg(feature = "ssao")]
            TemporalAntiAliasPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, daylight_cycle);
    }
}
