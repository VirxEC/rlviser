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
    // commands.spawn(DirectionalLightBundle {
    //     directional_light: DirectionalLight {
    //         illuminance: 100000.,
    //         shadows_enabled: true,
    //         ..default()
    //     },
    //     transform: Transform::from_xyz(1000000., 100000., 1000000.).looking_at(Vec3::ZERO, Vec3::Y),
    //     ..default()
    // });
    // commands.spawn(PointLightBundle::default());
    commands.spawn(SpotLightBundle {
        spot_light: SpotLight {
            range: 6000.,
            intensity: 5000.,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(0., 2000., 0.).looking_to(-Vec3::Y, Vec3::Y),
        ..default()
    });

    // commands.insert_resource(AmbientLight { brightness: 0.2, ..default() });

    let camera_start_pos = Vec3::new(0., 200., 0.);

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
