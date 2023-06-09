use std::time::Duration;

use bevy::prelude::*;
use bevy_atmosphere::prelude::*;
use bevy_framepace::{FramepacePlugin, FramepaceSettings};
use bevy_mod_picking::prelude::*;

use crate::spectator::*;

#[derive(Component)]
struct Sun;

#[derive(Resource)]
struct CycleTimer(Timer);

#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
pub enum PrimaryCamera {
    #[default]
    Spectator,
    Director(u32),
    TrackCar(u32),
}

fn setup(mut commands: Commands) {
    // lights in the goals
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            range: 10000.,
            radius: 100.,
            intensity: 10000000.,
            ..default()
        },
        transform: Transform::from_xyz(0., 300., 5500.),
        ..default()
    });

    commands.spawn(PointLightBundle {
        point_light: PointLight {
            range: 10000.,
            radius: 100.,
            intensity: 10000000.,
            ..default()
        },
        transform: Transform::from_xyz(0., 300., -5500.),
        ..default()
    });

    commands.insert_resource(AmbientLight {
        brightness: 0.3,
        ..default()
    });

    commands.spawn((DirectionalLightBundle::default(), Sun));

    commands.spawn((
        PrimaryCamera::default(),
        Camera3dBundle {
            projection: PerspectiveProjection {
                near: 5.,
                far: 500000.,
                ..default()
            }
            .into(),
            transform: Transform::from_translation(Vec3::new(-3000., 1000., 0.)).looking_to(Vec3::X, Vec3::Y),
            ..default()
        },
        AtmosphereCamera::default(),
        RaycastPickCamera::default(),
        Spectator,
    ));
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

        atmosphere.sun_position = Vec3::new(-t.cos(), t.sin(), 0.);

        if let Some((mut light_trans, mut directional)) = query.single_mut().into() {
            light_trans.translation = atmosphere.sun_position * 100000000.;
            light_trans.look_at(Vec3::ZERO, Vec3::Y);
            directional.illuminance = t.sin().max(0.0).powf(2.0) * 100000.;
        }
    }
}

#[derive(Component)]
pub struct EntityName {
    pub name: String,
}

impl EntityName {
    #[inline]
    pub fn new<T: ToString>(name: T) -> Self {
        Self { name: name.to_string() }
    }
}

#[derive(Component, Clone, Copy)]
pub struct HighlightedEntity;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpectatorSettings {
            base_speed: 25.,
            alt_speed: 10.,
            sensitivity: 0.75,
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
        .add_plugin(FramepacePlugin)
        .add_plugin(SpectatorPlugin)
        .add_plugin(AtmospherePlugin)
        .add_plugins(DefaultPickingPlugins)
        .add_startup_system(setup)
        .add_system(daylight_cycle);
    }
}
