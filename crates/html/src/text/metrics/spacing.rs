//! Resolve font-relative spacing and tab lengths using the same measured glyph metrics.
//!
//! `ic` uses the advance of 水, rather than `em` or the width of an ASCII
//! space. Returning zero caused a valid `tab-size: 1ic` to use the fallback
//! eight-space tab interval instead (CSS Values 4 §6.1.4, CSS Text 3 §tab-size).

use super::{ch_ex_px, ic_px};

/// Межбуквенный и межсловный интервал в точках.
///
/// Единицы шрифта (`ch`, `ex`) сюда входят наравне с `em`: пока они молча
/// отбрасывались, `word-spacing: -1ch` не действовал вовсе
/// (`word-spacing-002`). Доля берётся от кегля — как `em`: в модели ширина
/// пробела шрифта отдельно не хранится.
pub fn spacing_px(
    len: Option<crate::style::values::value::Len>,
    family: &str,
    size_px: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> f32 {
    use crate::style::values::value::Len;
    let (ch, ex) = ch_ex_px(family, size_px, vcx);
    match len {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size_px,
        Some(Len::Ch(k)) => k * ch,
        Some(Len::Ex(k)) => k * ex,
        Some(Len::Ic(k)) => k * ic_px(family, size_px, vcx),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn ic_spacing_uses_the_measured_ideographic_advance() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        super::super::install_probe(|_, size| (size * 0.25, size * 0.75, size, size * 1.5), vcx);
        assert_eq!(spacing_px(Some(Len::Ic(1.0)), "Probe", 20.0, vcx), 30.0);
        assert_eq!(spacing_px(Some(Len::Ic(0.5)), "Probe", 40.0, vcx), 30.0);
        assert_eq!(spacing_px(Some(Len::Ic(-1.0)), "Probe", 20.0, vcx), -30.0);
        assert_eq!(spacing_px(Some(Len::Ch(1.0)), "Probe", 20.0, vcx), 5.0);
    }

    #[test]
    fn ic_spacing_has_the_specified_em_fallback_without_a_probe() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        assert_eq!(spacing_px(Some(Len::Ic(1.0)), "Missing", 24.0, vcx), 24.0);
    }
}
