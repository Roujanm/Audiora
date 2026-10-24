//! Conversion between the engine's f32 samples and the integer/float layouts
//! audio hardware uses in WASAPI exclusive mode.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    /// 32-bit IEEE float (what shared mode always uses).
    F32,
    /// 32-bit signed integer, all bits valid.
    I32,
    /// 24 valid bits left-justified in a 32-bit container.
    I24In32,
    /// 24-bit packed little-endian.
    I24,
    I16,
}

impl SampleFormat {
    pub const ALL: [SampleFormat; 5] =
        [SampleFormat::F32, SampleFormat::I32, SampleFormat::I24In32, SampleFormat::I24, SampleFormat::I16];

    pub fn container_bytes(self) -> usize {
        match self {
            SampleFormat::F32 | SampleFormat::I32 | SampleFormat::I24In32 => 4,
            SampleFormat::I24 => 3,
            SampleFormat::I16 => 2,
        }
    }

    pub fn valid_bits(self) -> u16 {
        match self {
            SampleFormat::F32 | SampleFormat::I32 => 32,
            SampleFormat::I24In32 | SampleFormat::I24 => 24,
            SampleFormat::I16 => 16,
        }
    }

    pub fn is_float(self) -> bool {
        self == SampleFormat::F32
    }

    /// f32 samples -> device bytes (`dst.len()` must be `src.len() * container_bytes`).
    /// Values beyond full scale are clipped, never wrapped.
    pub fn encode(self, src: &[f32], dst: &mut [u8]) {
        debug_assert_eq!(dst.len(), src.len() * self.container_bytes());
        match self {
            SampleFormat::F32 => {
                for (s, d) in src.iter().zip(dst.chunks_exact_mut(4)) {
                    d.copy_from_slice(&s.to_le_bytes());
                }
            }
            SampleFormat::I32 => {
                for (s, d) in src.iter().zip(dst.chunks_exact_mut(4)) {
                    let v = (s.clamp(-1.0, 1.0) as f64 * 2_147_483_647.0).round() as i32;
                    d.copy_from_slice(&v.to_le_bytes());
                }
            }
            SampleFormat::I24In32 => {
                for (s, d) in src.iter().zip(dst.chunks_exact_mut(4)) {
                    let v = (s.clamp(-1.0, 1.0) as f64 * 8_388_607.0).round() as i32;
                    d.copy_from_slice(&(v << 8).to_le_bytes());
                }
            }
            SampleFormat::I24 => {
                for (s, d) in src.iter().zip(dst.chunks_exact_mut(3)) {
                    let v = (s.clamp(-1.0, 1.0) as f64 * 8_388_607.0).round() as i32;
                    d.copy_from_slice(&v.to_le_bytes()[..3]);
                }
            }
            SampleFormat::I16 => {
                for (s, d) in src.iter().zip(dst.chunks_exact_mut(2)) {
                    let v = (s.clamp(-1.0, 1.0) as f64 * 32_767.0).round() as i16;
                    d.copy_from_slice(&v.to_le_bytes());
                }
            }
        }
    }

    /// device bytes -> f32 samples.
    pub fn decode(self, src: &[u8], dst: &mut [f32]) {
        debug_assert_eq!(src.len(), dst.len() * self.container_bytes());
        match self {
            SampleFormat::F32 => {
                for (s, d) in src.chunks_exact(4).zip(dst.iter_mut()) {
                    *d = f32::from_le_bytes([s[0], s[1], s[2], s[3]]);
                }
            }
            SampleFormat::I32 => {
                for (s, d) in src.chunks_exact(4).zip(dst.iter_mut()) {
                    *d = (i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64 / 2_147_483_648.0) as f32;
                }
            }
            SampleFormat::I24In32 => {
                for (s, d) in src.chunks_exact(4).zip(dst.iter_mut()) {
                    *d = ((i32::from_le_bytes([s[0], s[1], s[2], s[3]]) >> 8) as f64 / 8_388_608.0) as f32;
                }
            }
            SampleFormat::I24 => {
                for (s, d) in src.chunks_exact(3).zip(dst.iter_mut()) {
                    // sign-extend through the top byte
                    let v = i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8;
                    *d = (v as f64 / 8_388_608.0) as f32;
                }
            }
            SampleFormat::I16 => {
                for (s, d) in src.chunks_exact(2).zip(dst.iter_mut()) {
                    *d = (i16::from_le_bytes([s[0], s[1]]) as f64 / 32_768.0) as f32;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Vec<f32> {
        (-100..=100).map(|i| i as f32 / 100.0).collect()
    }

    fn roundtrip(fmt: SampleFormat, input: &[f32]) -> Vec<f32> {
        let mut bytes = vec![0u8; input.len() * fmt.container_bytes()];
        fmt.encode(input, &mut bytes);
        let mut out = vec![0f32; input.len()];
        fmt.decode(&bytes, &mut out);
        out
    }

    #[test]
    fn roundtrip_error_is_within_one_lsb() {
        let input = ramp();
        for (fmt, lsb) in [
            (SampleFormat::F32, 0.0),
            (SampleFormat::I32, 1e-9),
            (SampleFormat::I24In32, 2.0 / 8_388_608.0),
            (SampleFormat::I24, 2.0 / 8_388_608.0),
            (SampleFormat::I16, 2.0 / 32_768.0),
        ] {
            let out = roundtrip(fmt, &input);
            let worst = input.iter().zip(&out).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
            assert!(worst <= lsb, "{fmt:?}: worst error {worst} > {lsb}");
        }
    }

    #[test]
    fn full_scale_and_silence() {
        for fmt in SampleFormat::ALL {
            let out = roundtrip(fmt, &[0.0, 1.0, -1.0]);
            assert_eq!(out[0], 0.0, "{fmt:?} silence");
            assert!((out[1] - 1.0).abs() < 1e-3 && (out[2] + 1.0).abs() < 1e-3, "{fmt:?} full scale {out:?}");
        }
    }

    #[test]
    fn overs_clip_instead_of_wrapping() {
        for fmt in [SampleFormat::I32, SampleFormat::I24In32, SampleFormat::I24, SampleFormat::I16] {
            let out = roundtrip(fmt, &[3.0, -3.0, f32::INFINITY]);
            assert!(out[0] > 0.99 && out[1] < -0.99 && out[2] > 0.99, "{fmt:?} {out:?}");
        }
    }

    #[test]
    fn packed_24_bit_byte_layout_is_little_endian() {
        let mut b = [0u8; 3];
        SampleFormat::I24.encode(&[0.5], &mut b);
        // 0.5 * 8388607 = 4194303.5 -> 4194304 = 0x400000
        assert_eq!(b, [0x00, 0x00, 0x40]);
        SampleFormat::I24.encode(&[-0.5], &mut b);
        assert_eq!(b, [0x00, 0x00, 0xC0]);
    }

    #[test]
    fn i24_in_32_is_left_justified() {
        let mut b = [0u8; 4];
        SampleFormat::I24In32.encode(&[0.5], &mut b);
        assert_eq!(b, [0x00, 0x00, 0x00, 0x40]);
        assert_eq!(SampleFormat::I24In32.valid_bits(), 24);
        assert_eq!(SampleFormat::I24In32.container_bytes(), 4);
    }
}
