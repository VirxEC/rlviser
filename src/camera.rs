use std::{f32::consts::PI, time::Duration};

use bevy::prelude::*;
use bevy_atmosphere::prelude::*;
use bevy_spectator::*;

#[derive(Component)]
struct Sun;

#[derive(Resource)]
struct CycleTimer(Timer);

fn setup(mut commands: Commands) {
    commands.spawn(SpotLightBundle {
        spot_light: SpotLight {
            range: 10000.,
            radius: 5000.,
            intensity: 20000000.,
            shadows_enabled: true,
            inner_angle: PI / 4.,
            outer_angle: PI / 3.,
            ..default()
        },
        transform: Transform::from_xyz(0., 2000., 4000.).looking_at(Vec3::new(0., 700., 5120.), Vec3::Z),
        ..default()
    });

    commands.spawn(SpotLightBundle {
        spot_light: SpotLight {
            range: 10000.,
            radius: 5000.,
            intensity: 20000000.,
            shadows_enabled: true,
            inner_angle: PI / 4.,
            outer_angle: PI / 3.,
            ..default()
        },
        transform: Transform::from_xyz(0., 2000., -4000.).looking_at(Vec3::new(0., 700., -5120.), Vec3::Z),
        ..default()
    });

    // lights in the goals
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            range: 10000.,
            radius: 100.,
            intensity: 10000000.,
            shadows_enabled: true,
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
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(0., 300., -5500.),
        ..default()
    });

    commands.insert_resource(AmbientLight { brightness: 0.2, ..default() });

    commands.spawn((DirectionalLightBundle::default(), Sun));

    commands.spawn((
        Camera3dBundle {
            projection: Projection::Perspective(PerspectiveProjection { far: 20000., ..default() }),
            transform: Transform::from_translation(Vec3::new(-3000., 1000., 0.)).looking_to(Vec3::X, Vec3::Y),
            ..default()
        },
        AtmosphereCamera::default(),
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

    if timer.0.finished() && !offset.stop_day {
        let t = (offset.offset + time.elapsed_seconds_wrapped()) / (200. / offset.day_speed);

        atmosphere.sun_position = Vec3::new(0., t.sin(), t.cos());

        if let Some((mut light_trans, mut directional)) = query.single_mut().into() {
            light_trans.rotation = Quat::from_rotation_x(-t.sin().atan2(t.cos()));
            directional.illuminance = t.sin().max(0.0).powf(2.0) * 100000.;
        }
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpectatorSettings {
            base_speed: 25.,
            alt_speed: 10.,
            sensitivity: 0.75,
            ..default()
        })
        .insert_resource(AtmosphereModel::default())
        .insert_resource(CycleTimer(Timer::new(Duration::from_secs_f32(1. / 60.), TimerMode::Repeating)))
        .insert_resource(DaylightOffset::default())
        .add_plugin(SpectatorPlugin)
        .add_plugin(AtmospherePlugin)
        .add_startup_system(setup)
        .add_system(daylight_cycle);
    }
}
