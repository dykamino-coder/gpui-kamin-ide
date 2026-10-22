//! Профили ICC (@color-profile): загрузка, теги, матрица и кривые, перевод в sRGB.

use super::*;
use crate::color::{curve_at, icc_curve, icc_matrix};
use crate::document::state::ValueCx;
use crate::tolerance::GAMUT_SLACK;

mod resources;
pub(crate) use resources::load_profiles_sheets;

/// Применить вшитый цветовой профиль ICC к готовому образу.
///
/// Понимается матричный профиль RGB (v2): колоранты `rXYZ/gXYZ/bXYZ` и
/// кривые `rTRC/gTRC/bTRC` (`curv`: линейная, гамма или таблица). Точки
/// линеаризуются кривыми, матрица ведёт в XYZ D50, дальше та же дорога, что
/// у `lab()`: D50 -> D65 -> линейный sRGB -> гамма. Не разобрался профиль —
/// `None`, образ остаётся как есть.
pub(crate) fn apply_icc(
    image: &std::sync::Arc<gpui::RenderImage>,
    profile: &[u8],
) -> Option<std::sync::Arc<gpui::RenderImage>> {
    let m = icc_matrix(profile)?;
    let curves = [
        icc_curve(profile, b"rTRC")?,
        icc_curve(profile, b"gTRC")?,
        icc_curve(profile, b"bTRC")?,
    ];
    let size = image.size(0);
    let (w, h) = (size.width.0 as u32, size.height.0 as u32);
    let bytes = image.as_bytes(0)?;
    let mut out = Vec::with_capacity(bytes.len());
    for px in bytes.chunks_exact(4) {
        // Порядок BGRA, цвета премультиплицированы; при полной непрозрачности
        // (обычный случай картинок-эталонов) это просто цвет.
        let a = px[3] as f32 / 255.0;
        let un = |v: u8| {
            if a > 0.0 {
                (v as f32 / 255.0 / a).min(1.0)
            } else {
                0.0
            }
        };
        let (b, g, r) = (un(px[0]), un(px[1]), un(px[2]));
        let lin = [
            curve_at(&curves[0], r),
            curve_at(&curves[1], g),
            curve_at(&curves[2], b),
        ];
        let xyz50 = mul(m, lin);
        let xyz = mul(D50_TO_D65, xyz50);
        let srgb = mul(XYZ_TO_LINEAR_SRGB, xyz);
        let (r, g, b) = (
            srgb_gamma(srgb[0]),
            srgb_gamma(srgb[1]),
            srgb_gamma(srgb[2]),
        );
        // Вблизи края охвата — простой зажим; охватное отображение — только
        // настоящему выходу за охват (почему именно столько — `tolerance`).
        let near = |v: f32| (-GAMUT_SLACK..=1.0 + GAMUT_SLACK).contains(&v);
        let (r, g, b) = if near(r) && near(g) && near(b) {
            (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
        } else {
            gamut_map(r, g, b)
        };
        out.push((b * a * 255.0).round() as u8);
        out.push((g * a * 255.0).round() as u8);
        out.push((r * a * 255.0).round() as u8);
        out.push(px[3]);
    }
    gpui::bgra_bytes_to_image(w, h, out)
}

/// Профиль `@color-profile` документа по имени (`--foo`): имена
/// придумывает страница, реестр — её (`document::state`).
pub(super) fn profile(name: &str, vcx: ValueCx<'_>) -> Option<Vec<u8>> {
    vcx.doc.profiles.borrow().get(name).cloned()
}

/// Прогнать цвет через профиль: кривые -> матрица -> XYZ D50 -> sRGB.
pub(super) fn icc_to_srgb(profile: &[u8], c: [f32; 3]) -> Option<(f32, f32, f32)> {
    let m = icc_matrix(profile)?;
    let curves = [
        icc_curve(profile, b"rTRC")?,
        icc_curve(profile, b"gTRC")?,
        icc_curve(profile, b"bTRC")?,
    ];
    let lin = [
        curve_at(&curves[0], c[0]),
        curve_at(&curves[1], c[1]),
        curve_at(&curves[2], c[2]),
    ];
    let xyz = mul(D50_TO_D65, mul(m, lin));
    let srgb = mul(XYZ_TO_LINEAR_SRGB, xyz);
    Some(gamut_map(
        srgb_gamma(srgb[0]),
        srgb_gamma(srgb[1]),
        srgb_gamma(srgb[2]),
    ))
}
