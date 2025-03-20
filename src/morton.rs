use bevy::math::Vec3A;

#[derive(Debug)]
pub struct Morton {
    offset: Vec3A,
    scale: Vec3A,
}

impl Default for Morton {
    fn default() -> Self {
        Self::new(Vec3A::splat(-10_000.), Vec3A::splat(10_000.))
    }
}

impl Morton {
    #[must_use]
    fn new(min: Vec3A, max: Vec3A) -> Self {
        // 2 ^ 20 - 1 = 1048575
        let scale = 1_048_575. / (max - min);

        Self { offset: min, scale }
    }

    /// Prepare a 21-bit unsigned int for inverweaving.
    #[must_use]
    pub fn expand3(a: u32) -> u64 {
        let mut x = u64::from(a) & 0x001f_ffff; // we only look at the first 21 bits

        x = (x | (x << 32)) & 0x001f_0000_0000_ffff;
        x = (x | (x << 16)) & 0x001f_0000_ff00_00ff;
        x = (x | (x << 8)) & 0x100f_00f0_0f00_f00f;
        x = (x | (x << 4)) & 0x10c3_0c30_c30c_30c3;
        x = (x | (x << 2)) & 0x1249_2492_4924_9249;

        x
    }

    /// Get an AABB's morton code.
    #[must_use]
    pub fn get_code(&self, point: Vec3A) -> u64 {
        let u = (point - self.offset) * self.scale;

        debug_assert!(u.x >= 0.);
        debug_assert!(u.y >= 0.);
        debug_assert!(u.z >= 0.);

        // These should actually be 21 bits, but there's no u21 type and the final type is u64 (21 bits * 3 = 63 bits)
        // Allowing these warnings is ok because:
        // We have offset the values so they're all greater than 0
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        (Self::expand3(u.x as u32) | (Self::expand3(u.y as u32) << 1) | (Self::expand3(u.z as u32) << 2))
    }
}
