use bevy::prelude::*;

pub struct GameOptions;

impl Plugin for GameOptions {
    fn build(&self, app: &mut App) {
        app.insert_resource(if cfg!(feature = "ssao") { Msaa::Off } else { Msaa::default() })
            .insert_resource(BallCam::default())
            .insert_resource(UiOverlayScale::default())
            .insert_resource(ShowTime::default())
            .insert_resource(GameSpeed::default())
            .insert_resource(MenuFocused::default())
            .insert_resource(CalcBallRot::default())
            .insert_resource(Extrapolation::default());
    }
}

#[derive(Resource, Default)]
pub struct Extrapolation(pub bool);

#[derive(Resource, PartialEq, Eq)]
pub struct MenuFocused(pub bool);

impl Default for MenuFocused {
    #[inline]
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource)]
pub struct CalcBallRot(pub bool);

impl Default for CalcBallRot {
    #[inline]
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Default)]
pub struct GameSpeed {
    pub paused: bool,
    pub speed: f32,
}

#[derive(Resource)]
pub struct BallCam {
    pub enabled: bool,
}

impl Default for BallCam {
    #[inline]
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Resource)]
pub struct ShowTime {
    pub enabled: bool,
}

impl Default for ShowTime {
    #[inline]
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Resource)]
pub struct UiOverlayScale {
    pub scale: f32,
}

impl Default for UiOverlayScale {
    #[inline]
    fn default() -> Self {
        Self { scale: 1. }
    }
}
