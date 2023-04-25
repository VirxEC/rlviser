use std::{f32::consts::PI, time::Duration};

use bevy::prelude::*;
use bevy_atmosphere::prelude::*;
use bevy_mod_picking::{CustomHighlightPlugin, DefaultPickingPlugins, HoverEvent, PickingCameraBundle, PickingEvent};

use crate::{spectator::*, udp::Car};

#[derive(Component)]
struct Sun;

#[derive(Resource)]
struct CycleTimer(Timer);

#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
pub enum PrimaryCamera {
    #[default]
    Spectator,
    TrackCar(u32),
}

fn setup(mut commands: Commands) {
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

    commands.insert_resource(AmbientLight { brightness: 0.1, ..default() });

    commands.spawn((DirectionalLightBundle::default(), Sun));

    commands
        .spawn((
            PrimaryCamera::default(),
            Camera3dBundle {
                projection: PerspectiveProjection { far: 500000., ..default() }.into(),
                transform: Transform::from_translation(Vec3::new(-3000., 1000., 0.)).looking_to(Vec3::X, Vec3::Y),
                ..default()
            },
            AtmosphereCamera::default(),
            Spectator,
        ))
        .insert(PickingCameraBundle::default());
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
    pub fn new<T: ToString>(name: T) -> Self {
        Self { name: name.to_string() }
    }
}

#[derive(Component)]
pub struct HighlightedEntity;

fn handle_picker_events(mut commands: Commands, mut events: EventReader<PickingEvent>) {
    for event in events.iter() {
        if let PickingEvent::Hover(hover) = event {
            match hover {
                HoverEvent::JustEntered(entity) => commands.entity(*entity).insert(HighlightedEntity),
                HoverEvent::JustLeft(entity) => commands.entity(*entity).remove::<HighlightedEntity>(),
            };
        }
    }
}

fn update_camera(car_query: Query<(&Car, &Transform)>, mut camera_query: Query<(&PrimaryCamera, &mut Transform), Without<Car>>) {
    let Ok((state, mut camera_transform)) = camera_query.get_single_mut() else {
        return;
    };

    let PrimaryCamera::TrackCar(car_id) = state else {
        return;
    };

    let Some(car_transform) = car_query.iter().find_map(|(car, car_transform)| {
        if car.id() == *car_id {
            Some(car_transform)
        } else {
            None
        }
    }) else {
        return;
    };

    camera_transform.translation = car_transform.translation - car_transform.right() * 300. + car_transform.up() * 150.;
    camera_transform.look_to(car_transform.forward(), car_transform.up());
    camera_transform.rotation *= Quat::from_rotation_y(-PI / 2.) * Quat::from_rotation_x(-PI / 16.);
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
        .add_plugins(DefaultPickingPlugins.build().disable::<CustomHighlightPlugin<StandardMaterial>>())
        .add_startup_system(setup)
        .add_system(handle_picker_events)
        .add_system(daylight_cycle)
        .add_system(update_camera);
    }
}
