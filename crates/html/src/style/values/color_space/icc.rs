//! Профили ICC (@color-profile): загрузка, теги, матрица и кривые, перевод в sRGB.

use crate::color::{IccProfile, clamp_near_gamut, gamut_map};
use crate::document::state::ValueCx;

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
    let profile = IccProfile::parse(profile)?;
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
        let (r, g, b) = clamp_near_gamut(profile.to_srgb_unmapped([r, g, b]));
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
    let [r, g, b] = IccProfile::parse(profile)?.to_srgb_unmapped(c);
    Some(gamut_map(r, g, b))
}
