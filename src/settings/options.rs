use crate::camera::PrimaryCamera;
use bevy::prelude::*;
use std::{
    fs,
    io::{self, Write},
};

pub struct GameOptions;

impl Plugin for GameOptions {
    fn build(&self, app: &mut App) {
        app.insert_resource(Options::default_read_file())
            .insert_resource(BallCam::default())
            .insert_resource(UiOverlayScale::default())
            .insert_resource(ShowTime::default())
            .insert_resource(GameSpeed::default())
            .insert_resource(MenuFocused::default())
            .insert_resource(CalcBallRot::default())
            .insert_resource(PacketSmoothing::default());
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Resource)]
pub struct Options {
    pub vsync: bool,
    pub uncap_fps: bool,
    pub fps_limit: f64,
    pub fps: (usize, [f32; 120]),
    pub ball_cam: bool,
    pub stop_day: bool,
    pub daytime: f32,
    pub day_speed: f32,
    pub msaa: usize,
    pub camera_state: PrimaryCamera,
    pub show_time: bool,
    pub ui_scale: f32,
    pub shadows: usize,
    pub game_speed: f32,
    pub paused: bool,
    pub mouse_sensitivity: f32,
    pub allow_rendering: bool,
    pub packet_smoothing: usize,
    pub calc_ball_rot: bool,
}

impl Default for Options {
    #[inline]
    fn default() -> Self {
        Self {
            vsync: false,
            uncap_fps: false,
            fps_limit: 120.,
            fps: (0, [0.; 120]),
            ball_cam: true,
            stop_day: true,
            daytime: 25.,
            day_speed: 1.,
            msaa: 2,
            camera_state: PrimaryCamera::Spectator,
            show_time: true,
            ui_scale: 1.,
            shadows: 0,
            game_speed: 1.,
            paused: false,
            mouse_sensitivity: 1.,
            allow_rendering: true,
            packet_smoothing: 1,
            calc_ball_rot: true,
        }
    }
}

impl Options {
    const FILE_NAME: &'static str = "settings.txt";

    #[inline]
    fn default_read_file() -> Self {
        Self::read_from_file().unwrap_or_else(|_| Self::create_file_from_defualt())
    }

    fn read_from_file() -> io::Result<Self> {
        let mut options = Self::default();

        let file = fs::read_to_string(Self::FILE_NAME)?;

        for line in file.lines() {
            let mut parts = line.split('=');

            let Some(key) = parts.next() else {
                continue;
            };

            let Some(value) = parts.next() else {
                continue;
            };

            match key {
                "vsync" => options.vsync = value.parse().unwrap(),
                "uncap_fps" => options.uncap_fps = value.parse().unwrap(),
                "fps_limit" => options.fps_limit = value.parse().unwrap(),
                "ball_cam" => options.ball_cam = value.parse().unwrap(),
                "stop_day" => options.stop_day = value.parse().unwrap(),
                "daytime" => options.daytime = value.parse().unwrap(),
                "day_speed" => options.day_speed = value.parse().unwrap(),
                "msaa" => options.msaa = value.parse().unwrap(),
                "camera_state" => options.camera_state = serde_json::from_str(value).unwrap(),
                "show_time" => options.show_time = value.parse().unwrap(),
                "ui_scale" => options.ui_scale = value.parse().unwrap(),
                "shadows" => options.shadows = value.parse().unwrap(),
                "game_speed" => options.game_speed = value.parse().unwrap(),
                "paused" => options.paused = value.parse().unwrap(),
                "mouse_sensitivity" => options.mouse_sensitivity = value.parse().unwrap(),
                "allow_rendering" => options.allow_rendering = value.parse().unwrap(),
                "packet_smoothing" => options.packet_smoothing = serde_json::from_str(value).unwrap(),
                "calc_ball_rot" => options.calc_ball_rot = value.parse().unwrap(),
                _ => println!("Unknown key {key} with value {value}"),
            }
        }

        Ok(options)
    }

    fn create_file_from_defualt() -> Self {
        let options = Self::default();

        if let Err(e) = options.write_options_to_file() {
            println!("Failed to create {} due to: {e}", Self::FILE_NAME);
        }

        options
    }

    pub fn write_options_to_file(&self) -> io::Result<()> {
        let mut file = fs::File::create(Self::FILE_NAME)?;

        file.write_fmt(format_args!("vsync={}\n", self.vsync))?;
        file.write_fmt(format_args!("uncap_fps={}\n", self.uncap_fps))?;
        file.write_fmt(format_args!("fps_limit={}\n", self.fps_limit))?;
        file.write_fmt(format_args!("ball_cam={}\n", self.ball_cam))?;
        file.write_fmt(format_args!("stop_day={}\n", self.stop_day))?;
        file.write_fmt(format_args!("daytime={}\n", self.daytime))?;
        file.write_fmt(format_args!("day_speed={}\n", self.day_speed))?;
        file.write_fmt(format_args!("msaa={}\n", self.msaa))?;
        file.write_fmt(format_args!("camera_state={}\n", serde_json::to_string(&self.camera_state)?))?;
        file.write_fmt(format_args!("show_time={}\n", self.show_time))?;
        file.write_fmt(format_args!("ui_scale={}\n", self.ui_scale))?;
        file.write_fmt(format_args!("shadows={}\n", self.shadows))?;
        file.write_fmt(format_args!("game_speed={}\n", self.game_speed))?;
        file.write_fmt(format_args!("paused={}\n", self.paused))?;
        file.write_fmt(format_args!("mouse_sensitivity={}\n", self.mouse_sensitivity))?;
        file.write_fmt(format_args!("allow_rendering={}\n", self.allow_rendering))?;
        file.write_fmt(format_args!("packet_smoothing={}\n", self.packet_smoothing))?;
        file.write_fmt(format_args!("calc_ball_rot={}\n", self.calc_ball_rot))?;

        Ok(())
    }

    #[inline]
    #[allow(clippy::float_cmp)]
    pub fn is_not_similar(&self, other: &Self) -> bool {
        self.vsync != other.vsync
            || self.uncap_fps != other.uncap_fps
            || self.fps_limit != other.fps_limit
            || self.ball_cam != other.ball_cam
            || self.stop_day != other.stop_day
            || self.daytime != other.daytime
            || self.day_speed != other.day_speed
            || self.msaa != other.msaa
            || self.camera_state != other.camera_state
            || self.show_time != other.show_time
            || self.ui_scale != other.ui_scale
            || self.shadows != other.shadows
            || self.game_speed != other.game_speed
            || self.paused != other.paused
            || self.mouse_sensitivity != other.mouse_sensitivity
            || self.allow_rendering != other.allow_rendering
            || self.packet_smoothing != other.packet_smoothing
            || self.calc_ball_rot != other.calc_ball_rot
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
