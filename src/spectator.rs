//! Copied over from `bevy_spectator` because base functions aren't public and changes are required to make it work nice

use crate::camera::PrimaryCamera;
use bevy::{
    input::mouse::MouseMotion,
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};

/// A marker `Component` for spectating cameras.
///
/// ## Usage
/// With the `init` feature:
/// - Add it to a single entity to mark it as a spectator.
/// - `init` will then find that entity and mark it as the active spectator in [`SpectatorSettings`].
///
/// (If there isn't a single [`Spectator`] (none or multiple, instead of one), there won't be an active spectator selected by the `init` feature.)
///
/// Without the `init` feature:
/// - Add it to entities to mark spectators.
/// - Manually alter [`SpectatorSettings`] to set the active spectator.
#[derive(Component)]
pub struct Spectator;

/// A `Plugin` for spectating your scene.
pub struct SpectatorPlugin;

impl Plugin for SpectatorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpectatorSettings>()
            .add_systems(PostStartup, spectator_init)
            .add_systems(Update, spectator_update);
    }
}

fn spectator_init(cameras: Query<Entity, With<Spectator>>, mut settings: ResMut<SpectatorSettings>) {
    use bevy::ecs::query::QuerySingleError;

    if settings.active_spectator.is_none() {
        settings.active_spectator = match cameras.get_single() {
            Ok(a) => Some(a),
            Err(QuerySingleError::NoEntities(_)) => {
                warn!("Failed to find a Spectator; Active camera will remain unset.");
                None
            }
            Err(QuerySingleError::MultipleEntities(_)) => {
                warn!("Found more than one Spectator; Active camera will remain unset.");
                None
            }
        };
    }
}

fn spectator_update(
    time: Res<Time>,
    keys: Res<Input<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    primary_camera: Query<&PrimaryCamera>,
    mut motion: EventReader<MouseMotion>,
    mut settings: ResMut<SpectatorSettings>,
    mut camera_transforms: Query<&mut Transform, With<Spectator>>,
) {
    let Some(camera_id) = settings.active_spectator else {
        motion.clear();
        return;
    };

    if primary_camera
        .get_single()
        .map(|state| *state != PrimaryCamera::Spectator)
        .unwrap_or_default()
    {
        motion.clear();
        return;
    }

    if let Ok(window) = windows.get_single() {
        if window.cursor.grab_mode == CursorGrabMode::None {
            motion.clear();
            return;
        }
    }

    let Ok(mut camera_transform) = camera_transforms.get_mut(camera_id) else {
        error!("Failed to find camera for active camera entity ({camera_id:?})");
        settings.active_spectator = None;
        motion.clear();
        return;
    };

    // rotation
    {
        let mouse_delta = motion.read().fold(Vec2::ZERO, |acc, d| acc + d.delta) * -settings.sensitivity;
        let (x, y, _) = camera_transform.rotation.to_euler(EulerRot::YXZ);

        camera_transform.rotation = Quat::from_euler(
            EulerRot::YXZ,
            x + mouse_delta.x,
            // At 90 degrees, yaw gets misinterpeted as roll. Making 89 the limit fixes that.
            (y + mouse_delta.y).clamp(-89f32.to_radians(), 89f32.to_radians()),
            0.,
        );
    }

    // translation
    {
        let forward = f32::from(keys.pressed(KeyCode::W));
        let backward = f32::from(keys.pressed(KeyCode::S));
        let right = f32::from(keys.pressed(KeyCode::D));
        let left = f32::from(keys.pressed(KeyCode::A));
        let up = f32::from(keys.pressed(KeyCode::Space));
        let down = f32::from(keys.pressed(KeyCode::ControlLeft));

        let speed = if keys.pressed(KeyCode::ShiftLeft) {
            settings.alt_speed
        } else {
            settings.base_speed
        };

        let delta_axial = (forward - backward) * speed;
        let delta_lateral = (right - left) * speed;
        let delta_vertical = (up - down) * speed;

        let mut forward = camera_transform.forward();
        forward.y = 0f32;
        forward = forward.normalize(); // fly fast even when look down/up

        let right = camera_transform.right();
        let up = Vec3::Y;

        let result = forward * delta_axial + right * delta_lateral + up * delta_vertical;

        camera_transform.translation += result * time.delta_seconds();
    }

    motion.clear();
}

/// A `Resource` for controlling [`Spectator`]s.
#[derive(Resource)]
pub struct SpectatorSettings {
    /// The `Entity` of the active [`Spectator`]. (Default: `None`)
    ///
    /// Use this to control which [`Spectator`] you are using.
    ///
    /// If the `init` feature is enabled, `None` will update to a single, marked camera.
    ///
    /// Setting to `None` will disable the spectator mode.
    pub active_spectator: Option<Entity>,
    /// The `Entity` of the active `Window`. (Default: `None`)
    ///
    /// Use this to control which `Window` will grab your mouse/hide the cursor.
    ///
    /// If `None`, the primary window will be used.
    pub active_window: Option<Entity>,
    /// The base speed of the active [`Spectator`]. (Default: `0.1`)
    ///
    /// Use this to control how fast the [`Spectator`] normally moves.
    pub base_speed: f32,
    /// The alternate speed of the active [`Spectator`]. (Default: `0.5`)
    ///
    /// Use this to control how fast the [`Spectator`] moves when you hold `Shift`.
    pub alt_speed: f32,
    /// The camera sensitivity of the active [`Spectator`]. (Default: `0.1`)
    ///
    /// Use this to control how fast the [`Spectator`] turns when you move the mouse.
    pub sensitivity: f32,
}

impl Default for SpectatorSettings {
    fn default() -> Self {
        Self {
            active_spectator: None,
            active_window: None,
            base_speed: 0.1,
            alt_speed: 0.5,
            sensitivity: 0.1,
        }
    }
}
