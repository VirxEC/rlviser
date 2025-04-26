use bevy::{
    math::{Mat3A as RotMat, Vec3A as Vec3},
    prelude::*,
};

#[repr(u8)]
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameMode {
    Soccar = 0,
    Hoops,
    Heatseeker,
    Snowday,
    Dropshot,
    #[default]
    TheVoid,
}

impl TryFrom<u8> for GameMode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Soccar),
            1 => Ok(Self::Hoops),
            2 => Ok(Self::Heatseeker),
            3 => Ok(Self::Snowday),
            4 => Ok(Self::Dropshot),
            5 => Ok(Self::TheVoid),
            _ => Err(()),
        }
    }
}

impl TryFrom<u8> for Team {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Blue),
            1 => Ok(Self::Orange),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct BallHitInfo {
    pub is_valid: bool,
    pub relative_pos_on_ball: Vec3,
    pub ball_pos: Vec3,
    pub extra_hit_vel: Vec3,
    pub tick_count_when_hit: u64,
    pub tick_count_when_extra_impulse_applied: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct HeatseekerInfo {
    /// Which net the ball should seek towards;
    /// When 0, no net
    pub y_target_dir: f32,
    pub cur_target_speed: f32,
    pub time_since_hit: f32,
}

impl Default for HeatseekerInfo {
    #[inline]
    fn default() -> Self {
        Self {
            y_target_dir: 0.,
            cur_target_speed: 2900.,
            time_since_hit: 0.,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DropshotInfo {
    /// Charge level number, which controls the radius of damage when hitting tiles
    /// 1 = damages r=1 -> 1 tile
    /// 2 = damages r=2 -> 7 tiles
    /// 3 = damages r=3 -> 19 tiles
    pub charge_level: i32,
    /// Resets when a tile is damaged
    pub accumulated_hit_force: f32,
    /// Which side of the field the ball can damage (0=none, -1=blue, 1=orange)
    pub y_target_dir: f32,
    pub has_damaged: bool,
    /// Only valid if `has_damaged`
    pub last_damage_tick: u64,
}

impl Default for DropshotInfo {
    #[inline]
    fn default() -> Self {
        Self {
            charge_level: 1,
            accumulated_hit_force: 0.,
            y_target_dir: 0.,
            has_damaged: false,
            last_damage_tick: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BallState {
    pub tick_count_since_update: u64,
    pub pos: Vec3,
    pub rot_mat: RotMat,
    pub vel: Vec3,
    pub ang_vel: Vec3,
    pub hs_info: HeatseekerInfo,
    pub ds_info: DropshotInfo,
}

impl Default for BallState {
    #[inline]
    fn default() -> Self {
        Self {
            tick_count_since_update: 0,
            pos: Vec3::new(0., 0., 93.15),
            rot_mat: RotMat::IDENTITY,
            vel: Vec3::ZERO,
            ang_vel: Vec3::ZERO,
            hs_info: HeatseekerInfo::default(),
            ds_info: DropshotInfo::default(),
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub enum Team {
    #[default]
    Blue,
    Orange,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct WheelPairConfig {
    pub wheel_radius: f32,
    pub suspension_rest_length: f32,
    pub connection_point_offset: Vec3,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct CarConfig {
    pub hitbox_size: Vec3,
    pub hitbox_pos_offset: Vec3,
    pub front_wheels: WheelPairConfig,
    pub back_wheels: WheelPairConfig,
    pub three_wheels: bool,
    pub dodge_deadzone: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CarControls {
    pub throttle: f32,
    pub steer: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
    pub boost: bool,
    pub jump: bool,
    pub handbrake: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WorldContact {
    pub has_contact: bool,
    pub contact_normal: Vec3,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CarContact {
    pub other_car_id: u32,
    pub cooldown_timer: f32,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CarState {
    pub pos: Vec3,
    pub rot_mat: RotMat,
    pub vel: Vec3,
    pub ang_vel: Vec3,
    pub tick_count_since_update: u64,
    pub is_on_ground: bool,
    pub wheels_with_contact: [bool; 4],
    pub has_jumped: bool,
    pub has_double_jumped: bool,
    pub has_flipped: bool,
    pub flip_rel_torque: Vec3,
    pub jump_time: f32,
    pub flip_time: f32,
    pub is_flipping: bool,
    pub is_jumping: bool,
    pub air_time: f32,
    pub air_time_since_jump: f32,
    pub boost: f32,
    pub time_since_boosted: f32,
    pub is_boosting: bool,
    pub boosting_time: f32,
    pub is_supersonic: bool,
    pub supersonic_time: f32,
    pub handbrake_val: f32,
    pub is_auto_flipping: bool,
    pub auto_flip_timer: f32,
    pub auto_flip_torque_scale: f32,
    pub world_contact: WorldContact,
    pub car_contact: CarContact,
    pub is_demoed: bool,
    pub demo_respawn_timer: f32,
    pub ball_hit_info: BallHitInfo,
    pub last_controls: CarControls,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct CarInfo {
    pub id: u32,
    pub team: Team,
    pub state: CarState,
    pub config: CarConfig,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct BoostPadState {
    pub is_active: bool,
    pub cooldown: f32,
    pub cur_locked_car_id: u32,
    pub prev_locked_car_id: u32,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct BoostPad {
    pub is_big: bool,
    pub position: Vec3,
    pub state: BoostPadState,
}

#[repr(u8)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum TileState {
    #[default]
    Full,
    Damaged,
    Broken,
}

impl TryFrom<u8> for TileState {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Full),
            1 => Ok(Self::Damaged),
            2 => Ok(Self::Broken),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct DropshotTile {
    pub pos: Vec3,
    pub state: TileState,
}

#[derive(Clone, Resource, Default, Debug)]
pub struct GameState {
    pub tick_count: u64,
    pub tick_rate: f32,
    pub game_mode: GameMode,
    pub ball: BallState,
    pub pads: Box<[BoostPad]>,
    pub cars: Box<[CarInfo]>,
    pub tiles: [Vec<DropshotTile>; 2],
}
