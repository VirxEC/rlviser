//! # bevy_spectator
//!
//! A spectator camera plugin for the [Bevy game engine](https://bevyengine.org/).
//!
//! ## Controls
//!
//! |Action|Key|
//! |-|-|
//! |Forward|`W`|
//! |Left|`A`|
//! |Backward|`S`|
//! |Right|`D`|
//! |Up|`Space`|
//! |Down|`LControl`|
//! |Alternative Speed|`LShift`|
//! |Release Cursor|`Escape`|
//!
//! Movement is constrained to the appropriate axes. (`WASD` to X & Z axes, `Space` & `LShift` to the Y axis)
//!
//! ## `basic` Example
//! ```
//! use bevy::prelude::*;
//! use bevy_spectator::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugin(SpectatorPlugin)
//!         .add_startup_system(setup)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands) {
//!     commands.spawn((
//!         Camera3dBundle::default(), Spectator
//!     ));
//! }
//! ```

// Copied over because base functions aren't public and changes are required to make it work nice

use bevy::{
    input::mouse::MouseMotion,
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};

use crate::camera::PrimaryCamera;

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
        app.init_resource::<SpectatorSettings>();

        app.add_startup_system(spectator_init.in_base_set(StartupSet::PostStartup));

        app.add_system(spectator_update);
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

#[allow(clippy::too_many_arguments)]
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
        let mouse_delta = {
            let mut total = Vec2::ZERO;
            for d in motion.iter() {
                total += d.delta;
            }
            total
        };

        let mouse_x = -mouse_delta.x * time.delta_seconds() * settings.sensitivity;
        let mouse_y = -mouse_delta.y * time.delta_seconds() * settings.sensitivity;

        let mut dof: Vec3 = camera_transform.rotation.to_euler(EulerRot::YXZ).into();

        dof.x += mouse_x;
        // At 90 degrees, yaw gets misinterpeted as roll. Making 89 the limit fixes that.
        dof.y = (dof.y + mouse_y).clamp(-89f32.to_radians(), 89f32.to_radians());
        dof.z = 0f32;

        camera_transform.rotation = Quat::from_euler(EulerRot::YXZ, dof.x, dof.y, dof.z);
    }

    // translation
    {
        let forward = if keys.pressed(KeyCode::W) { 1f32 } else { 0f32 };
        let backward = if keys.pressed(KeyCode::S) { 1f32 } else { 0f32 };
        let right = if keys.pressed(KeyCode::D) { 1f32 } else { 0f32 };
        let left = if keys.pressed(KeyCode::A) { 1f32 } else { 0f32 };
        let up = if keys.pressed(KeyCode::Space) { 1f32 } else { 0f32 };
        let down = if keys.pressed(KeyCode::LControl) { 1f32 } else { 0f32 };

        let speed = if keys.pressed(KeyCode::LShift) {
            settings.alt_speed
        } else {
            settings.base_speed
        };

        let delta_axial = (forward - backward) * speed;
        let delta_lateral = (right - left) * speed;
        let delta_vertical = (up - down) * speed;

        let mut forward = camera_transform.forward();
        forward.y = 0f32;
        forward = forward.normalize_or_zero(); // fly fast even when look down/up

        let mut right = camera_transform.right();
        right.y = 0f32; // more of a sanity check
        let up = Vec3::Y;

        camera_transform.translation += forward * delta_axial + right * delta_lateral + up * delta_vertical;
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
