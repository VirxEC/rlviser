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
            .insert_resource(PacketSmoothing::default());
    }
}

#[derive(Clone, Copy, Resource, Default)]
pub enum PacketSmoothing {
    None,
    #[default]
    Interpolate,
    Extrapolate,
}

impl PacketSmoothing {
    pub fn from_usize(value: usize) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Interpolate,
            2 => Self::Extrapolate,
            _ => unreachable!(),
        }
    }
}

#[derive(Resource, PartialEq, Eq, DerefMut, Deref)]
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
