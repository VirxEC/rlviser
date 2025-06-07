use crate::{
    renderer::{CustomColor as Color, Render, RenderMessage},
    rocketsim::{
        BallHitInfo, BallState, BoostPad, BoostPadState, CarConfig, CarContact, CarControls, CarInfo, CarState,
        DropshotInfo, DropshotTile, GameMode, GameState, HeatseekerInfo, Team, TileState, WheelPairConfig, WorldContact,
    },
};
use bevy::math::{Mat3A as RotMat, Vec2, Vec3 as BVec3, Vec3A as Vec3};
use core::fmt;

pub trait FromBytes {
    fn from_bytes(bytes: &[u8]) -> Self;
}

pub trait FromBytesExact: FromBytes {
    const NUM_BYTES: usize;
}

struct ByteReader<'a> {
    idx: usize,
    bytes: &'a [u8],
}

impl<'a> ByteReader<'a> {
    #[inline]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { idx: 0, bytes }
    }

    #[track_caller]
    pub fn read<I: FromBytesExact>(&mut self) -> I {
        let item = I::from_bytes(&self.bytes[self.idx..self.idx + I::NUM_BYTES]);
        self.idx += I::NUM_BYTES;
        item
    }
}

impl Drop for ByteReader<'_> {
    fn drop(&mut self) {
        debug_assert_eq!(self.idx, self.bytes.len(), "ByteReader dropped with unread data");
    }
}

impl FromBytes for bool {
    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        bytes[0] != 0
    }
}

impl FromBytesExact for bool {
    const NUM_BYTES: usize = 1;
}

impl FromBytesExact for f32 {
    const NUM_BYTES: usize = 4;
}

impl FromBytes for f32 {
    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}

impl FromBytesExact for u8 {
    const NUM_BYTES: usize = 1;
}

impl FromBytes for u8 {
    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        bytes[0]
    }
}

impl FromBytesExact for u16 {
    const NUM_BYTES: usize = 2;
}

impl FromBytes for u16 {
    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self::from_le_bytes([bytes[0], bytes[1]])
    }
}

impl FromBytesExact for u32 {
    const NUM_BYTES: usize = 4;
}

impl FromBytes for u32 {
    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}

impl FromBytesExact for u64 {
    const NUM_BYTES: usize = 8;
}

impl FromBytes for u64 {
    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]])
    }
}

impl FromBytesExact for i32 {
    const NUM_BYTES: usize = 4;
}

impl FromBytes for i32 {
    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        Self::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
}

impl<T: FromBytesExact + fmt::Debug, const N: usize> FromBytesExact for [T; N] {
    const NUM_BYTES: usize = T::NUM_BYTES * N;
}

impl<T: FromBytesExact + fmt::Debug, const N: usize> FromBytes for [T; N] {
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut reader = ByteReader::new(bytes);

        let items = (0..N).map(|_| reader.read()).collect::<Vec<T>>();
        items.try_into().unwrap()
    }
}

macro_rules! impl_from_bytes_exact_for_enums {
    ($($variant:ident),*) => {
        $(
            impl FromBytesExact for $variant {
                const NUM_BYTES: usize = 1;
            }

            impl FromBytes for $variant {
                #[inline]
                fn from_bytes(bytes: &[u8]) -> Self {
                    Self::try_from(bytes[0]).unwrap()
                }
            }
        )*
    };
}

impl_from_bytes_exact_for_enums!(Team, GameMode, TileState);

impl FromBytesExact for Vec3 {
    const NUM_BYTES: usize = f32::NUM_BYTES * 3;
}

impl FromBytes for Vec3 {
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut reader = ByteReader::new(bytes);
        Self::new(reader.read(), reader.read(), reader.read())
    }
}

macro_rules! impl_from_bytes_exact {
    ($t:ty, $n:expr, $($p:ident),+) => {
        impl FromBytes for $t {
            fn from_bytes(bytes: &[u8]) -> Self {
                let mut reader = ByteReader::new(bytes);
                Self {
                    $($p: reader.read(),)+
                }
            }
        }

        impl FromBytesExact for $t {
            const NUM_BYTES: usize = $n;
        }
    };
}

pub trait ToBytesExact<const N: usize>: FromBytesExact {
    fn to_bytes(&self) -> [u8; N];
}

struct ByteWriter<const N: usize> {
    idx: usize,
    bytes: [u8; N],
}

impl<const N: usize> ByteWriter<N> {
    #[inline]
    pub const fn new() -> Self {
        Self { idx: 0, bytes: [0; N] }
    }

    pub fn write<I: ToBytesExact<M>, const M: usize>(&mut self, item: &I) {
        self.bytes[self.idx..self.idx + M].copy_from_slice(&item.to_bytes());
        self.idx += M;
    }

    #[inline]
    pub fn inner(self) -> [u8; N] {
        debug_assert_eq!(self.idx, N, "ByteWriter::inner() called before all bytes were written");
        self.bytes
    }
}

macro_rules! impl_to_bytes_for_array_size {
    ($([$t:ty; $N:expr]),+) => {
        $(impl ToBytesExact<{ <$t>::NUM_BYTES * $N }> for [$t; $N] {
            fn to_bytes(&self) -> [u8; <$t>::NUM_BYTES * $N] {
                let mut writer = ByteWriter::<{ <$t>::NUM_BYTES * $N }>::new();
                for item in self {
                    writer.write(item);
                }
                writer.inner()
            }
        })+
    }
}

impl_to_bytes_for_array_size!([bool; 4]);

macro_rules! impl_to_bytes_exact_via_std {
    ($($t:ty),+) => {
        $(impl ToBytesExact<{ Self::NUM_BYTES }> for $t {
            fn to_bytes(&self) -> [u8; Self::NUM_BYTES] {
                self.to_le_bytes()
            }
        })+
    };
}

impl_to_bytes_exact_via_std!(u8, u16, u32, u64, i32, f32);

macro_rules! impl_to_bytes_exact_as_u8 {
    ($($t:ty),+) => {
        $(impl ToBytesExact<{ Self::NUM_BYTES }> for $t {
            fn to_bytes(&self) -> [u8; Self::NUM_BYTES] {
                [*self as u8]
            }
        })+
    };
}

impl_to_bytes_exact_as_u8!(bool, Team, GameMode, TileState);

macro_rules! impl_to_bytes_exact {
    ($t:ty, $($p:ident),+) => {
        impl ToBytesExact<{ Self::NUM_BYTES }> for $t {
            fn to_bytes(&self) -> [u8; Self::NUM_BYTES] {
                let mut writer = ByteWriter::<{ Self::NUM_BYTES }>::new();
                $(writer.write(&self.$p);)+
                writer.inner()
            }
        }
    };
}

impl_to_bytes_exact!(Vec3, x, y, z);

macro_rules! impl_bytes_exact {
    ($t:ty, $n:expr, $($p:ident),+) => {
        impl_from_bytes_exact!($t, $n, $($p),+);
        impl_to_bytes_exact!($t, $($p),+);
    };
}

impl_bytes_exact!(Vec2, f32::NUM_BYTES * 2, x, y);
impl_bytes_exact!(BVec3, f32::NUM_BYTES * 3, x, y, z);
impl_bytes_exact!(Color, f32::NUM_BYTES * 4, r, g, b, a);
impl_bytes_exact!(RotMat, Vec3::NUM_BYTES * 3, x_axis, y_axis, z_axis);
impl_bytes_exact!(
    HeatseekerInfo,
    f32::NUM_BYTES * 3,
    y_target_dir,
    cur_target_speed,
    time_since_hit
);
impl_bytes_exact!(
    DropshotInfo,
    i32::NUM_BYTES + f32::NUM_BYTES * 2 + 1 + u64::NUM_BYTES,
    charge_level,
    accumulated_hit_force,
    y_target_dir,
    has_damaged,
    last_damage_tick
);
impl_bytes_exact!(
    BallState,
    u64::NUM_BYTES + Vec3::NUM_BYTES * 3 + RotMat::NUM_BYTES + HeatseekerInfo::NUM_BYTES + DropshotInfo::NUM_BYTES,
    tick_count_since_update,
    pos,
    rot_mat,
    vel,
    ang_vel,
    hs_info,
    ds_info
);
impl_bytes_exact!(
    BoostPadState,
    1 + f32::NUM_BYTES + u32::NUM_BYTES * 2,
    is_active,
    cooldown,
    cur_locked_car_id,
    prev_locked_car_id
);
impl_bytes_exact!(
    BoostPad,
    1 + Vec3::NUM_BYTES + BoostPadState::NUM_BYTES,
    is_big,
    position,
    state
);
impl_bytes_exact!(
    BallHitInfo,
    1 + Vec3::NUM_BYTES * 3 + u64::NUM_BYTES * 2,
    is_valid,
    relative_pos_on_ball,
    ball_pos,
    extra_hit_vel,
    tick_count_when_hit,
    tick_count_when_extra_impulse_applied
);
impl_bytes_exact!(
    CarControls,
    f32::NUM_BYTES * 5 + 3,
    throttle,
    steer,
    pitch,
    yaw,
    roll,
    boost,
    jump,
    handbrake
);
impl_bytes_exact!(WorldContact, 1 + Vec3::NUM_BYTES, has_contact, contact_normal);
impl_bytes_exact!(CarContact, u32::NUM_BYTES + f32::NUM_BYTES, other_car_id, cooldown_timer);
impl_bytes_exact!(
    CarState,
    u64::NUM_BYTES
        + Vec3::NUM_BYTES * 4
        + RotMat::NUM_BYTES
        + 14
        + f32::NUM_BYTES * 12
        + WorldContact::NUM_BYTES
        + CarContact::NUM_BYTES
        + BallHitInfo::NUM_BYTES
        + CarControls::NUM_BYTES,
    tick_count_since_update,
    pos,
    rot_mat,
    vel,
    ang_vel,
    is_on_ground,
    wheels_with_contact,
    has_jumped,
    has_double_jumped,
    has_flipped,
    flip_rel_torque,
    jump_time,
    flip_time,
    is_flipping,
    is_jumping,
    air_time,
    air_time_since_jump,
    boost,
    time_since_boosted,
    is_boosting,
    boosting_time,
    is_supersonic,
    supersonic_time,
    handbrake_val,
    is_auto_flipping,
    auto_flip_timer,
    auto_flip_torque_scale,
    world_contact,
    car_contact,
    is_demoed,
    demo_respawn_timer,
    ball_hit_info,
    last_controls
);
impl_bytes_exact!(
    WheelPairConfig,
    f32::NUM_BYTES * 2 + Vec3::NUM_BYTES,
    wheel_radius,
    suspension_rest_length,
    connection_point_offset
);
impl_bytes_exact!(
    CarConfig,
    Vec3::NUM_BYTES * 2 + WheelPairConfig::NUM_BYTES * 2 + f32::NUM_BYTES + 1,
    hitbox_size,
    hitbox_pos_offset,
    front_wheels,
    back_wheels,
    three_wheels,
    dodge_deadzone
);
impl_bytes_exact!(
    CarInfo,
    u32::NUM_BYTES + Team::NUM_BYTES + CarState::NUM_BYTES + CarConfig::NUM_BYTES,
    id,
    team,
    state,
    config
);
impl_bytes_exact!(DropshotTile, Vec3::NUM_BYTES + 1, pos, state);

impl Render {
    fn count_bytes(&self) -> usize {
        match self {
            Self::Line2D { .. } => 1 + Vec2::NUM_BYTES * 2 + Color::NUM_BYTES,
            Self::Line { .. } => 1 + Vec3::NUM_BYTES * 2 + Color::NUM_BYTES,
            Self::LineStrip { positions, .. } => 1 + u16::NUM_BYTES + positions.len() * Vec3::NUM_BYTES + Color::NUM_BYTES,
        }
    }

    fn from_reader(reader: &mut ByteReader) -> Self {
        match reader.read::<u8>() {
            0 => Self::Line2D {
                start: reader.read(),
                end: reader.read(),
                color: reader.read(),
            },
            1 => Self::Line {
                start: reader.read(),
                end: reader.read(),
                color: reader.read(),
            },
            2 => Self::LineStrip {
                positions: (0..reader.read::<u16>()).map(|_| reader.read()).collect(),
                color: reader.read(),
            },
            _ => unreachable!(),
        }
    }
}

impl ToBytes for Render {
    fn to_bytes(&self) -> Vec<u8> {
        let num_bytes = self.count_bytes();
        let mut bytes = Vec::with_capacity(num_bytes);

        match self {
            Self::Line2D { start, end, color } => {
                bytes.push(0);
                bytes.extend_from_slice(&start.to_bytes());
                bytes.extend_from_slice(&end.to_bytes());
                bytes.extend_from_slice(&color.to_bytes());
            }
            Self::Line { start, end, color } => {
                bytes.push(1);
                bytes.extend_from_slice(&start.to_bytes());
                bytes.extend_from_slice(&end.to_bytes());
                bytes.extend_from_slice(&color.to_bytes());
            }
            Self::LineStrip { positions, color } => {
                bytes.push(2);
                bytes.extend_from_slice(&(positions.len() as u16).to_bytes());

                for pos in positions {
                    bytes.extend_from_slice(&pos.to_bytes());
                }

                bytes.extend_from_slice(&color.to_bytes());
            }
        }

        debug_assert_eq!(bytes.len(), num_bytes);

        bytes
    }
}

impl FromBytes for RenderMessage {
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut reader = ByteReader::new(bytes);
        reader.read::<u32>();

        match reader.read::<u8>() {
            0 => Self::AddRender(
                reader.read(),
                (0..reader.read::<u16>()).map(|_| Render::from_reader(&mut reader)).collect(),
            ),
            1 => Self::RemoveRender(reader.read()),
            _ => unreachable!(),
        }
    }
}

impl RenderMessage {
    pub const MIN_NUM_BYTES: usize = u32::NUM_BYTES;

    fn count_bytes(&self) -> usize {
        match self {
            Self::AddRender(_, renders) => {
                Self::MIN_NUM_BYTES
                    + i32::NUM_BYTES
                    + 1
                    + u16::NUM_BYTES
                    + renders.iter().map(Render::count_bytes).sum::<usize>()
            }
            Self::RemoveRender(_) => Self::MIN_NUM_BYTES + i32::NUM_BYTES,
        }
    }

    pub fn get_num_bytes(bytes: &[u8]) -> usize {
        u32::from_bytes(&bytes[..u32::NUM_BYTES]) as usize
    }
}

impl ToBytes for RenderMessage {
    fn to_bytes(&self) -> Vec<u8> {
        let num_bytes = self.count_bytes();
        let mut bytes = Vec::with_capacity(num_bytes);
        bytes.extend_from_slice(&(num_bytes as u32).to_bytes());

        match self {
            Self::AddRender(id, renders) => {
                bytes.push(0);
                bytes.extend_from_slice(&id.to_bytes());
                bytes.extend_from_slice(&(renders.len() as u16).to_bytes());
                bytes.extend(renders.iter().flat_map(ToBytes::to_bytes));
            }
            Self::RemoveRender(id) => {
                bytes.push(1);
                bytes.extend_from_slice(&id.to_bytes());
            }
        }

        debug_assert_eq!(bytes.len(), num_bytes);

        bytes
    }
}

impl FromBytes for GameState {
    #[inline]
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut reader = ByteReader::new(bytes);

        let tick_count = reader.read();
        let tick_rate = reader.read();
        let game_mode = reader.read();
        let num_pads = reader.read();
        let num_cars = reader.read();

        Self {
            tick_count,
            tick_rate,
            game_mode,
            ball: reader.read(),
            pads: (0..num_pads).map(|_| reader.read()).collect(),
            cars: (0..num_cars).map(|_| reader.read()).collect(),
            tiles: [
                (0..70).map(|_| reader.read()).collect(),
                (0..70).map(|_| reader.read()).collect(),
            ],
        }
    }
}

impl GameState {
    pub const MIN_NUM_BYTES: usize = u64::NUM_BYTES + f32::NUM_BYTES + 1 + u32::NUM_BYTES * 2;

    const fn count_bytes(&self) -> usize {
        Self::MIN_NUM_BYTES
            + BallState::NUM_BYTES
            + self.pads.len() * BoostPad::NUM_BYTES
            + self.cars.len() * CarInfo::NUM_BYTES
            + DropshotTile::NUM_BYTES * 140
    }

    #[inline]
    pub fn get_num_bytes(bytes: &[u8]) -> usize {
        Self::MIN_NUM_BYTES
            + BallState::NUM_BYTES
            + Self::read_num_pads(bytes) * BoostPad::NUM_BYTES
            + Self::read_num_cars(bytes) * CarInfo::NUM_BYTES
            + DropshotTile::NUM_BYTES * 140
    }

    #[inline]
    pub fn read_tick_count(bytes: &[u8]) -> u64 {
        u64::from_bytes(&bytes[..u64::NUM_BYTES])
    }

    #[inline]
    pub fn read_num_pads(bytes: &[u8]) -> usize {
        u32::from_bytes(&bytes[u64::NUM_BYTES + f32::NUM_BYTES + 1..u64::NUM_BYTES + f32::NUM_BYTES + 1 + u32::NUM_BYTES])
            as usize
    }

    #[inline]
    pub fn read_num_cars(bytes: &[u8]) -> usize {
        u32::from_bytes(&bytes[u64::NUM_BYTES + f32::NUM_BYTES + 1 + u32::NUM_BYTES..Self::MIN_NUM_BYTES]) as usize
    }
}

pub trait ToBytes {
    fn to_bytes(&self) -> Vec<u8>;
}

impl ToBytes for GameState {
    fn to_bytes(&self) -> Vec<u8> {
        let num_bytes = self.count_bytes();
        let mut bytes = Vec::with_capacity(num_bytes);

        bytes.extend(self.tick_count.to_bytes());
        bytes.extend(self.tick_rate.to_bytes());
        bytes.extend(self.game_mode.to_bytes());
        bytes.extend(&(self.pads.len() as u32).to_bytes());
        bytes.extend(&(self.cars.len() as u32).to_bytes());
        bytes.extend(self.ball.to_bytes());
        bytes.extend(self.pads.iter().flat_map(ToBytesExact::<{ BoostPad::NUM_BYTES }>::to_bytes));
        bytes.extend(self.cars.iter().flat_map(ToBytesExact::<{ CarInfo::NUM_BYTES }>::to_bytes));
        bytes.extend(
            self.tiles[0]
                .iter()
                .flat_map(ToBytesExact::<{ DropshotTile::NUM_BYTES }>::to_bytes),
        );
        bytes.extend(
            self.tiles[1]
                .iter()
                .flat_map(ToBytesExact::<{ DropshotTile::NUM_BYTES }>::to_bytes),
        );

        debug_assert_eq!(num_bytes, bytes.len());
        bytes
    }
}
