//! Разобранный матричный RGB-профиль ICC (v2: колоранты и кривые `curv`) и перевод цвета
//! из его пространства в sRGB: кривые → матрица в XYZ D50 → D65 → линейный sRGB → гамма,
//! та же дорога, что у `lab()`. Один разбор на профиль; читают `@color-profile` стиля и
//! вшитые профили картинок.

use super::icc::{IccCurve, curve_at, icc_curve, icc_matrix};
use super::{D50_TO_D65, XYZ_TO_LINEAR_SRGB, gamut_map, mul, srgb_gamma};
use crate::tolerance::GAMUT_SLACK;

/// Матричный RGB-профиль: колоранты в PCS D50 и три тоновые кривые.
pub(crate) struct IccProfile {
    matrix: [f32; 9],
    curves: [IccCurve; 3],
}

impl IccProfile {
    /// Разобрать профиль; `None`, если это не матричный RGB-профиль с кривыми `curv`.
    pub(crate) fn parse(profile: &[u8]) -> Option<Self> {
        Some(Self {
            matrix: icc_matrix(profile)?,
            curves: [
                icc_curve(profile, b"rTRC")?,
                icc_curve(profile, b"gTRC")?,
                icc_curve(profile, b"bTRC")?,
            ],
        })
    }

    /// Цвет профиля (каналы 0..1) в sRGB с гаммой, без втягивания в охват.
    pub(crate) fn to_srgb_unmapped(&self, c: [f32; 3]) -> [f32; 3] {
        let lin = [
            curve_at(&self.curves[0], c[0]),
            curve_at(&self.curves[1], c[1]),
            curve_at(&self.curves[2], c[2]),
        ];
        let xyz = mul(D50_TO_D65, mul(self.matrix, lin));
        let srgb = mul(XYZ_TO_LINEAR_SRGB, xyz);
        [
            srgb_gamma(srgb[0]),
            srgb_gamma(srgb[1]),
            srgb_gamma(srgb[2]),
        ]
    }
}

/// Цвет точки картинки после перевода профиля: вблизи края охвата — простой зажим,
/// охватное отображение — только настоящему выходу за охват (почему именно столько —
/// `tolerance`).
pub(crate) fn clamp_near_gamut([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let near = |v: f32| (-GAMUT_SLACK..=1.0 + GAMUT_SLACK).contains(&v);
    if near(r) && near(g) && near(b) {
        (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
    } else {
        gamut_map(r, g, b)
    }
}
