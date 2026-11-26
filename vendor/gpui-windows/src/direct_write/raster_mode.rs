//! KaminIDE patch: DirectWrite rendering mode and rasterization size of a
//! glyph chosen as Skia's DirectWrite scaler chooses them in Chromium, so the
//! glyph coverage is the one Chrome gets from DirectWrite.
//!
//! A `gasp` table of version 1 decides between natural and natural-symmetric
//! smoothing for the size; without one, glyphs above 20 px or of fonts without
//! TrueType instructions are smoothed symmetrically, and small hinted glyphs
//! (often tuned only for GDI ClearType) use natural smoothing at the size
//! rounded to 1/64 px.

use windows::Win32::Graphics::DirectWrite::*;

const fn tag(t: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*t)
}

/// What a face's `gasp` and `maxp` tables say about its rasterization.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RasterHints {
    /// `gasp` version and its (max ppem, behavior flags) ranges.
    gasp: Option<(u16, Vec<(u16, u16)>)>,
    /// The face carries TrueType instructions.
    hinted: bool,
}

const GASP_SYMMETRIC_SMOOTHING: u16 = 0x0008;

fn be16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]))
}

fn parse_gasp(data: &[u8]) -> Option<(u16, Vec<(u16, u16)>)> {
    let version = be16(data, 0)?;
    if version > 1 {
        return None;
    }
    let count = be16(data, 2)? as usize;
    let ranges = (0..count)
        .map(|i| Some((be16(data, 4 + 4 * i)?, be16(data, 6 + 4 * i)?)))
        .collect::<Option<Vec<_>>>()?;
    Some((version, ranges))
}

/// `maxp` version 1.0 (TrueType outlines) with a non-zero
/// `maxSizeOfInstructions`.
fn parse_hinted(data: &[u8]) -> bool {
    data.len() >= 32 && data[..4] == [0, 1, 0, 0] && be16(data, 26).is_some_and(|n| n != 0)
}

fn with_table<T>(
    face: &IDWriteFontFace3,
    name: &[u8; 4],
    read: impl FnOnce(&[u8]) -> T,
) -> Option<T> {
    let mut data = std::ptr::null_mut();
    let mut size = 0u32;
    let mut context = std::ptr::null_mut();
    let mut exists = false.into();
    unsafe { face.TryGetFontTable(tag(name), &mut data, &mut size, &mut context, &mut exists) }
        .ok()?;
    if !exists.as_bool() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, size as usize) };
    let value = read(bytes);
    unsafe { face.ReleaseFontTable(context) };
    Some(value)
}

impl RasterHints {
    pub(crate) fn read(face: &IDWriteFontFace3) -> Self {
        Self {
            gasp: with_table(face, b"gasp", parse_gasp).flatten(),
            hinted: with_table(face, b"maxp", parse_hinted).unwrap_or(false),
        }
    }

    /// Smoothing mode and the em size to rasterize at for a glyph of
    /// `device_size` device pixels.
    pub(crate) fn mode(&self, device_size: f32) -> (DWRITE_RENDERING_MODE1, f32) {
        let gdi_size = match (device_size * 64.0).round() / 64.0 {
            0.0 => 1.0,
            size => size,
        };
        if let Some((version, ranges)) = &self.gasp
            && *version >= 1
        {
            let ppem = gdi_size.round() as i32;
            let mut min = -1;
            for (max, flags) in ranges {
                if min < ppem && ppem <= *max as i32 {
                    let mode = if flags & GASP_SYMMETRIC_SMOOTHING != 0 {
                        DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC
                    } else {
                        DWRITE_RENDERING_MODE1_NATURAL
                    };
                    return (mode, device_size);
                }
                min = *max as i32;
            }
        }
        if device_size > 20.0 || !self.hinted {
            (DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC, device_size)
        } else {
            (DWRITE_RENDERING_MODE1_NATURAL, gdi_size)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gasp(version: u16, ranges: &[(u16, u16)]) -> Vec<u8> {
        let mut data = [version, ranges.len() as u16]
            .map(u16::to_be_bytes)
            .concat();
        for (max, flags) in ranges {
            data.extend(max.to_be_bytes());
            data.extend(flags.to_be_bytes());
        }
        data
    }

    #[test]
    fn version_1_gasp_picks_symmetric_smoothing_per_range() {
        let hints = RasterHints {
            gasp: parse_gasp(&gasp(1, &[(8, 0x2), (16, 0x1 | 0x2), (0xffff, 0xf)])),
            hinted: true,
        };
        assert_eq!(hints.mode(12.0), (DWRITE_RENDERING_MODE1_NATURAL, 12.0));
        assert_eq!(hints.mode(16.4), (DWRITE_RENDERING_MODE1_NATURAL, 16.4));
        assert_eq!(
            hints.mode(17.0),
            (DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC, 17.0)
        );
    }

    #[test]
    fn without_version_1_gasp_small_hinted_glyphs_use_natural_at_gdi_size() {
        for gasp_table in [None, parse_gasp(&gasp(0, &[(0xffff, 0x3)]))] {
            let hints = RasterHints {
                gasp: gasp_table,
                hinted: true,
            };
            let (mode, size) = hints.mode(13.333_333);
            assert_eq!(mode, DWRITE_RENDERING_MODE1_NATURAL);
            assert_eq!(size, (13.333_333f32 * 64.0).round() / 64.0);
            assert_eq!(
                hints.mode(24.0),
                (DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC, 24.0)
            );
        }
        let unhinted = RasterHints {
            gasp: None,
            hinted: false,
        };
        assert_eq!(
            unhinted.mode(12.0),
            (DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC, 12.0)
        );
    }

    #[test]
    fn instructions_are_read_from_truetype_maxp_only() {
        let mut maxp = vec![0u8; 32];
        maxp[..4].copy_from_slice(&[0, 1, 0, 0]);
        assert!(!parse_hinted(&maxp));
        maxp[27] = 9;
        assert!(parse_hinted(&maxp));
        maxp[..4].copy_from_slice(&[0, 0, 0x50, 0]);
        assert!(!parse_hinted(&maxp[..6]));
        assert_eq!(parse_gasp(&gasp(2, &[(1, 1)])), None);
        assert_eq!(parse_gasp(&[0, 1, 0, 2, 0, 8]), None);
    }
}
