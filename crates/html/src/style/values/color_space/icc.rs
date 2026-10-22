//! Профили ICC (@color-profile): загрузка, теги, матрица и кривые, перевод в sRGB.

use crate::color::{IccProfile, gamut_map};
use crate::document::state::ValueCx;

mod resources;
pub(crate) use resources::load_profiles_sheets;

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
