use bevy::{
    math::{Mat3A, Vec3A},
    prelude::*,
};

use crate::udp::{BallState, BoostPadInfo, BoostPadState, CarConfig, CarInfo, CarState, GameState, Team};

trait ToBytesExact<const N: usize> {
    fn to_bytes(&self) -> [u8; N];
}

pub trait ToBytesVec {
    fn to_bytes(&self) -> Vec<u8>;
}

const VEC_SIZE: usize = 12;

impl ToBytesExact<VEC_SIZE> for Vec3 {
    fn to_bytes(&self) -> [u8; VEC_SIZE] {
        let mut bytes = [0; VEC_SIZE];
        bytes[..4].copy_from_slice(&self.x.to_ne_bytes());
        bytes[4..8].copy_from_slice(&self.y.to_ne_bytes());
        bytes[8..].copy_from_slice(&self.z.to_ne_bytes());
        bytes
    }
}

impl ToBytesExact<VEC_SIZE> for Vec3A {
    fn to_bytes(&self) -> [u8; VEC_SIZE] {
        let mut bytes = [0; VEC_SIZE];
        bytes[..4].copy_from_slice(&self.x.to_ne_bytes());
        bytes[4..8].copy_from_slice(&self.y.to_ne_bytes());
        bytes[8..].copy_from_slice(&self.z.to_ne_bytes());
        bytes
    }
}

const ROTMAT_SIZE: usize = VEC_SIZE * 3;

impl ToBytesExact<ROTMAT_SIZE> for Mat3A {
    fn to_bytes(&self) -> [u8; ROTMAT_SIZE] {
        let mut bytes = [0; ROTMAT_SIZE];
        bytes[..VEC_SIZE].copy_from_slice(&self.x_axis.to_bytes());
        bytes[VEC_SIZE..VEC_SIZE * 2].copy_from_slice(&self.y_axis.to_bytes());
        bytes[VEC_SIZE * 2..].copy_from_slice(&self.z_axis.to_bytes());
        bytes
    }
}

const BALL_STATE_SIZE: usize = VEC_SIZE * 3;

impl ToBytesExact<BALL_STATE_SIZE> for BallState {
    fn to_bytes(&self) -> [u8; BALL_STATE_SIZE] {
        let mut bytes = [0; BALL_STATE_SIZE];
        bytes[..VEC_SIZE].copy_from_slice(&self.pos.to_bytes());
        bytes[VEC_SIZE..VEC_SIZE * 2].copy_from_slice(&self.vel.to_bytes());
        bytes[VEC_SIZE * 2..].copy_from_slice(&self.ang_vel.to_bytes());
        bytes
    }
}

const CAR_STATE_SIZE: usize = VEC_SIZE + ROTMAT_SIZE;

impl ToBytesExact<CAR_STATE_SIZE> for CarState {
    fn to_bytes(&self) -> [u8; CAR_STATE_SIZE] {
        let mut bytes = [0; CAR_STATE_SIZE];
        bytes[..VEC_SIZE].copy_from_slice(&self.pos.to_bytes());
        bytes[VEC_SIZE..CAR_STATE_SIZE].copy_from_slice(&self.rot_mat.to_bytes());
        bytes
    }
}

const CAR_CONFIG_SIZE: usize = VEC_SIZE;

impl ToBytesExact<CAR_CONFIG_SIZE> for CarConfig {
    fn to_bytes(&self) -> [u8; CAR_CONFIG_SIZE] {
        let mut bytes = [0; CAR_CONFIG_SIZE];
        bytes[..].copy_from_slice(&self.hitbox_size.to_bytes());
        bytes
    }
}

const CAR_INFO_SIZE: usize = 5 + CAR_STATE_SIZE + CAR_CONFIG_SIZE;

impl ToBytesExact<CAR_INFO_SIZE> for CarInfo {
    fn to_bytes(&self) -> [u8; CAR_INFO_SIZE] {
        let mut bytes = [0; CAR_INFO_SIZE];
        bytes[0..4].copy_from_slice(&self.id.to_ne_bytes());
        bytes[4..5].copy_from_slice(&(self.team as u8).to_ne_bytes());
        bytes[5..(5 + CAR_STATE_SIZE)].copy_from_slice(&self.state.to_bytes());
        bytes[(5 + CAR_STATE_SIZE)..CAR_INFO_SIZE].copy_from_slice(&self.config.to_bytes());
        bytes
    }
}

const PAD_STATE_SIZE: usize = 13;

impl ToBytesExact<PAD_STATE_SIZE> for BoostPadState {
    fn to_bytes(&self) -> [u8; PAD_STATE_SIZE] {
        let mut bytes = [0; PAD_STATE_SIZE];
        bytes[..1].copy_from_slice(&(self.is_active as u8).to_ne_bytes());
        bytes[1..5].copy_from_slice(&self.cooldown.to_ne_bytes());
        bytes[5..9].copy_from_slice(&self.cur_locked_car_id.to_ne_bytes());
        bytes[9..].copy_from_slice(&self.prev_locked_car_id.to_ne_bytes());
        bytes
    }
}

const PAD_SIZE: usize = 1 + VEC_SIZE + PAD_STATE_SIZE;

impl ToBytesExact<PAD_SIZE> for BoostPadInfo {
    fn to_bytes(&self) -> [u8; PAD_SIZE] {
        let mut bytes = [0; PAD_SIZE];
        bytes[..1].copy_from_slice(&(self.is_big as u8).to_ne_bytes());
        bytes[1..1 + VEC_SIZE].copy_from_slice(&self.position.to_bytes());
        bytes[1 + VEC_SIZE..PAD_SIZE].copy_from_slice(&self.state.to_bytes());
        bytes
    }
}

impl ToBytesVec for GameState {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16 + BALL_STATE_SIZE + self.pads.len() * PAD_SIZE + self.cars.len() * CAR_INFO_SIZE);
        bytes.extend(self.tick_count.to_ne_bytes());
        bytes.extend(&(self.pads.len() as u32).to_ne_bytes());
        bytes.extend(&(self.cars.len() as u32).to_ne_bytes());
        bytes.extend(self.ball.to_bytes());
        bytes.extend(self.pads.iter().flat_map(ToBytesExact::<PAD_SIZE>::to_bytes));
        bytes.extend(self.cars.iter().flat_map(ToBytesExact::<CAR_INFO_SIZE>::to_bytes));

        bytes
    }
}

pub trait FromBytes {
    const NUM_BYTES: usize;
    fn from_bytes(bytes: &[u8]) -> Self;
}

impl FromBytes for f32 {
    const NUM_BYTES: usize = 4;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}

impl FromBytes for u32 {
    const NUM_BYTES: usize = 4;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}

impl FromBytes for u64 {
    const NUM_BYTES: usize = 8;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        u64::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]])
    }
}

impl FromBytes for Vec3 {
    const NUM_BYTES: usize = f32::NUM_BYTES * 3;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Vec3::new(
            f32::from_bytes(&bytes[..f32::NUM_BYTES]),
            f32::from_bytes(&bytes[f32::NUM_BYTES..f32::NUM_BYTES * 2]),
            f32::from_bytes(&bytes[f32::NUM_BYTES * 2..]),
        )
    }
}

impl FromBytes for Vec3A {
    const NUM_BYTES: usize = f32::NUM_BYTES * 3;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Vec3A::new(
            f32::from_bytes(&bytes[..f32::NUM_BYTES]),
            f32::from_bytes(&bytes[f32::NUM_BYTES..f32::NUM_BYTES * 2]),
            f32::from_bytes(&bytes[f32::NUM_BYTES * 2..]),
        )
    }
}

impl FromBytes for Mat3A {
    const NUM_BYTES: usize = Vec3A::NUM_BYTES * 3;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Mat3A::from_cols(
            Vec3A::from_bytes(&bytes[..Vec3A::NUM_BYTES]),
            Vec3A::from_bytes(&bytes[Vec3A::NUM_BYTES..Vec3A::NUM_BYTES * 2]),
            Vec3A::from_bytes(&bytes[Vec3A::NUM_BYTES * 2..]),
        )
    }
}

impl FromBytes for BallState {
    const NUM_BYTES: usize = Vec3::NUM_BYTES * 3;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            pos: Vec3::from_bytes(&bytes[..Vec3::NUM_BYTES]),
            vel: Vec3::from_bytes(&bytes[Vec3::NUM_BYTES..Vec3::NUM_BYTES * 2]),
            ang_vel: Vec3::from_bytes(&bytes[Vec3::NUM_BYTES * 2..]),
        }
    }
}

impl FromBytes for BoostPadState {
    const NUM_BYTES: usize = 1 + f32::NUM_BYTES + u32::NUM_BYTES * 2;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            is_active: bytes[0] != 0,
            cooldown: f32::from_bytes(&bytes[1..1 + f32::NUM_BYTES]),
            cur_locked_car_id: u32::from_bytes(&bytes[1 + f32::NUM_BYTES..1 + f32::NUM_BYTES + u32::NUM_BYTES]),
            prev_locked_car_id: u32::from_bytes(&bytes[1 + f32::NUM_BYTES + u32::NUM_BYTES..]),
        }
    }
}

impl FromBytes for BoostPadInfo {
    const NUM_BYTES: usize = 1 + Vec3::NUM_BYTES + BoostPadState::NUM_BYTES;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            is_big: bytes[0] != 0,
            position: Vec3::from_bytes(&bytes[1..1 + Vec3::NUM_BYTES]),
            state: BoostPadState::from_bytes(&bytes[1 + Vec3::NUM_BYTES..Self::NUM_BYTES]),
        }
    }
}

impl FromBytes for Team {
    const NUM_BYTES: usize = 1;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        match bytes[0] {
            0 => Team::Blue,
            1 => Team::Orange,
            _ => unreachable!(),
        }
    }
}

impl FromBytes for CarState {
    const NUM_BYTES: usize = Vec3::NUM_BYTES + Mat3A::NUM_BYTES;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            pos: Vec3::from_bytes(&bytes[..Vec3::NUM_BYTES]),
            rot_mat: Mat3A::from_bytes(&bytes[Vec3::NUM_BYTES..Self::NUM_BYTES]),
        }
    }
}

impl FromBytes for CarConfig {
    const NUM_BYTES: usize = Vec3::NUM_BYTES;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            hitbox_size: Vec3::from_bytes(bytes),
        }
    }
}

impl FromBytes for CarInfo {
    const NUM_BYTES: usize = u32::NUM_BYTES + Team::NUM_BYTES + CarState::NUM_BYTES + CarConfig::NUM_BYTES;

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            id: u32::from_bytes(&bytes[..u32::NUM_BYTES]),
            team: Team::from_bytes(&bytes[u32::NUM_BYTES..u32::NUM_BYTES + Team::NUM_BYTES]),
            state: CarState::from_bytes(&bytes[u32::NUM_BYTES + Team::NUM_BYTES..u32::NUM_BYTES + Team::NUM_BYTES + CarState::NUM_BYTES]),
            config: CarConfig::from_bytes(&bytes[u32::NUM_BYTES + Team::NUM_BYTES + CarState::NUM_BYTES..]),
        }
    }
}
