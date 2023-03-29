use std::f32::consts::PI;

use bevy::{
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};
use bevy_atmosphere::prelude::*;
use smooth_bevy_cameras::{
    controllers::fps::{FpsCameraBundle, FpsCameraController, FpsCameraPlugin},
    LookTransformPlugin,
};

fn grab_mouse(mut windows: Query<&mut Window, With<PrimaryWindow>>, mouse: Res<Input<MouseButton>>, key: Res<Input<KeyCode>>) {
    let mut window = windows.single_mut();
    if mouse.just_pressed(MouseButton::Left) {
        window.cursor.visible = false;
        window.cursor.grab_mode = if cfg!(windows) { CursorGrabMode::Confined } else { CursorGrabMode::Locked };
    }
    if key.just_pressed(KeyCode::Escape) {
        window.cursor.visible = true;
        window.cursor.grab_mode = CursorGrabMode::None;
    }
}

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

    // light in the middle of the field
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            range: 10000.,
            radius: 5000.,
            intensity: 10000000.,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(0., 1200., 0.),
        ..default()
    });

    commands.insert_resource(AmbientLight { brightness: 0.5, ..default() });

    let camera_start_pos = Vec3::new(-3000., 1000., 0.);

    commands
        .spawn(FpsCameraBundle::new(
            FpsCameraController {
                enabled: true,
                mouse_rotate_sensitivity: Vec2::splat(0.75),
                translate_sensitivity: 2000.0,
                smoothing_weight: 0.9,
            },
            camera_start_pos,
            camera_start_pos + Vec3::X,
            Vec3::Y,
        ))
        .insert(Camera3dBundle {
            projection: Projection::Perspective(PerspectiveProjection { far: 20000., ..default() }),
            transform: Transform::from_translation(camera_start_pos).looking_to(Vec3::X, Vec3::Y),
            ..default()
        })
        .insert(AtmosphereCamera::default());
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(LookTransformPlugin)
            .add_plugin(AtmospherePlugin)
            .add_plugin(FpsCameraPlugin::default())
            .add_startup_system(setup)
            .add_system(grab_mouse);
    }
}
