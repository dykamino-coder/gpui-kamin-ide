//! Совместимость CSS: преобразовать уникальный decoder RGBA в straight BGRA.
//! Порядок alpha/ICC и округление сохраняют прежний CSS upload, без второго растра.

use super::types::ColourSpace;
use crate::color::{IccProfile, clamp_near_gamut};

pub(super) fn css_bgra(bytes: &mut [u8], colour_space: &ColourSpace) {
    for p in bytes.chunks_exact_mut(4) {
        p.swap(0, 2);
        if p[3] > 0 {
            let a = p[3] as f32 / 255.0;
            for c in &mut p[..3] {
                *c = (*c as f32 / a) as u8;
            }
        }
    }
    if let ColourSpace::Embedded(profile) = colour_space
        && let Some(profile) = IccProfile::parse(profile)
    {
        for p in bytes.chunks_exact_mut(4) {
            let a = p[3] as f32 / 255.0;
            let un = |v: u8| {
                if a > 0.0 {
                    (v as f32 / 255.0 / a).min(1.0)
                } else {
                    0.0
                }
            };
            let (r, g, b) =
                clamp_near_gamut(profile.to_srgb_unmapped([un(p[2]), un(p[1]), un(p[0])]));
            p[..3].copy_from_slice(&[
                (b * a * 255.0).round() as u8,
                (g * a * 255.0).round() as u8,
                (r * a * 255.0).round() as u8,
            ]);
        }
    }
}
