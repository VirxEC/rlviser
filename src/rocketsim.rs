use bevy::{
    math::{Mat3A as RotMat, Vec3A as Vec3},
    prelude::*,
};

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
pub struct BallState {
    pub pos: Vec3,
    pub vel: Vec3,
    pub ang_vel: Vec3,
}

impl Default for BallState {
    fn default() -> Self {
        Self {
            pos: Vec3::new(0., 0., 92.),
            vel: Vec3::ZERO,
            ang_vel: Vec3::ZERO,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Default, Debug)]
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
pub struct CarState {
    pub pos: Vec3,
    pub rot_mat: RotMat,
    pub vel: Vec3,
    pub ang_vel: Vec3,
    pub is_on_ground: bool,
    pub has_jumped: bool,
    pub has_double_jumped: bool,
    pub has_flipped: bool,
    pub last_rel_dodge_torque: Vec3,
    pub jump_time: f32,
    pub flip_time: f32,
    pub is_jumping: bool,
    pub air_time_since_jump: f32,
    pub boost: f32,
    pub time_spent_boosting: f32,
    pub is_supersonic: bool,
    pub supersonic_time: f32,
    pub handbrake_val: f32,
    pub is_auto_flipping: bool,
    pub auto_flip_timer: f32,
    pub auto_flip_torque_scale: f32,
    pub has_contact: bool,
    pub contact_normal: Vec3,
    pub other_car_id: u32,
    pub cooldown_timer: f32,
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

#[derive(Resource, Default, Debug)]
pub struct GameState {
    pub tick_count: u64,
    pub tick_rate: f32,
    pub ball: BallState,
    pub pads: Vec<BoostPad>,
    pub cars: Vec<CarInfo>,
}
