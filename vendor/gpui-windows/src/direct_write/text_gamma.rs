//! KaminIDE patch: glyph coverage correction tables as Chromium on Windows
//! builds them (Skia's mask gamma: a 3-bit text-luminance row per 8-bit
//! coverage), so a glyph mask blends to the same RGB as in Chrome.
//!
//! Each row is computed for a text luminance `src` assuming the perceptual
//! inverse as background (`dst = 1 - src`): coverage gets a contrast boost that
//! fades out as the background guess gets dark, the boosted coverage mixes
//! `src` and `dst` in linear light, and the table stores the coverage that a
//! plain (non-linear) blend needs to reproduce that mix.

/// Number of luminance rows: Skia keeps 3 bits of text luminance.
pub(crate) const ROWS: usize = 8;

/// Contrast and device gamma used for the tables. `gamma == 0.0` is sRGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextGammaSettings {
    pub contrast: f32,
    pub gamma: f32,
}

impl TextGammaSettings {
    /// Chromium's Windows build defaults (`SK_GAMMA_SRGB`, contrast 1.0).
    pub(crate) const CHROMIUM_DEFAULT: Self = Self {
        contrast: 1.0,
        gamma: 0.0,
    };

    /// Skia stores contrast in 1/255 and gamma in 1/64 steps; the tables are
    /// built from the stored values, so the settings are quantized the same way.
    pub(crate) fn quantized(self) -> Self {
        let contrast = (self.contrast.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        let gamma = (self.gamma.clamp(0.0, 4.0 - f32::EPSILON) * 64.0) as u8;
        Self {
            contrast: contrast as f32 / 255.0,
            gamma: gamma as f32 / 64.0,
        }
    }
}

/// 3-bit luminance level expanded to 8 bits by bit replication.
pub(crate) fn expand_level(level: usize) -> u32 {
    let base = (level as u32) << 5;
    (base | (base >> 3) | (base >> 6)) & 0xff
}

fn to_linear(gamma: f32, v: f32) -> f32 {
    if gamma == 0.0 {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    } else if gamma == 1.0 {
        v
    } else {
        v.powf(gamma)
    }
}

fn from_linear(gamma: f32, v: f32) -> f32 {
    if gamma == 0.0 {
        if v <= 0.003_130_8 {
            v * 12.92
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    } else if gamma == 1.0 {
        v
    } else {
        v.powf(1.0 / gamma)
    }
}

fn round_byte(v: f32) -> u8 {
    (255.0 * v + 0.5).floor().clamp(0.0, 255.0) as u8
}

/// One correction row for the 8-bit text luminance `src_lum`.
pub(crate) fn row(src_lum: u32, settings: TextGammaSettings) -> [u8; 256] {
    let TextGammaSettings { contrast, gamma } = settings;
    let src = src_lum as f32 / 255.0;
    let lin_src = to_linear(gamma, src);
    let dst = 1.0 - src;
    let lin_dst = to_linear(gamma, dst);
    let adjusted_contrast = contrast * lin_dst;
    let mut table = [0u8; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let raw = i as f32 / 255.0;
        let boosted = raw + (1.0 - raw) * adjusted_contrast * raw;
        // Near-equal src and dst make the inverse blend unstable; keep only
        // the contrast boost there.
        *slot = if (src - dst).abs() < 1.0 / 256.0 {
            round_byte(boosted)
        } else {
            let lin_out = lin_src * boosted + (1.0 - boosted) * lin_dst;
            round_byte((from_linear(gamma, lin_out) - dst) / (src - dst))
        };
    }
    table
}

/// All rows packed four bytes per word (row-major, byte `i` of a row in bits
/// `8 * (i % 4)` of word `i / 4`), the layout the text shaders read.
pub(crate) fn packed_tables(settings: TextGammaSettings) -> [u32; ROWS * 64] {
    let settings = settings.quantized();
    let mut words = [0u32; ROWS * 64];
    for level in 0..ROWS {
        let row = row(expand_level(level), settings);
        for (i, byte) in row.iter().enumerate() {
            words[level * 64 + i / 4] |= (*byte as u32) << (8 * (i % 4));
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unpack(words: &[u32; ROWS * 64], level: usize, i: usize) -> u8 {
        (words[level * 64 + i / 4] >> (8 * (i % 4))) as u8
    }

    #[test]
    fn levels_expand_by_bit_replication() {
        let levels: Vec<u32> = (0..ROWS).map(expand_level).collect();
        assert_eq!(levels, [0, 36, 73, 109, 146, 182, 219, 255]);
    }

    #[test]
    fn rows_keep_empty_and_full_coverage() {
        let words = packed_tables(TextGammaSettings::CHROMIUM_DEFAULT);
        for level in 0..ROWS {
            assert_eq!(unpack(&words, level, 0), 0);
            assert_eq!(unpack(&words, level, 255), 255);
            for i in 1..256 {
                assert!(unpack(&words, level, i) >= unpack(&words, level, i - 1));
            }
        }
    }

    #[test]
    fn dark_text_thins_and_light_text_thickens_partial_coverage() {
        // Values of the Chromium default tables (checked against Chrome frames
        // of black and white text on the opposite background).
        let words = packed_tables(TextGammaSettings::CHROMIUM_DEFAULT);
        assert_eq!(unpack(&words, 0, 32), 28);
        assert_eq!(unpack(&words, 0, 128), 119);
        assert_eq!(unpack(&words, 7, 32), 99);
        assert_eq!(unpack(&words, 7, 128), 188);
    }

    #[test]
    fn settings_are_quantized_like_the_stored_ones() {
        let q = TextGammaSettings {
            contrast: 0.5,
            gamma: 1.8,
        }
        .quantized();
        assert_eq!(q.contrast, 128.0 / 255.0);
        assert_eq!(q.gamma, 115.0 / 64.0);
        assert_eq!(
            TextGammaSettings::CHROMIUM_DEFAULT.quantized(),
            TextGammaSettings::CHROMIUM_DEFAULT
        );
    }

    #[test]
    fn linear_gamma_without_contrast_is_identity() {
        let identity = row(
            expand_level(3),
            TextGammaSettings {
                contrast: 0.0,
                gamma: 1.0,
            },
        );
        assert!(identity.iter().enumerate().all(|(i, v)| *v as usize == i));
    }
}
