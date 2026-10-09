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
pub fn spacing_px(len: Option<crate::value::Len>, family: &str, size_px: f32) -> f32 {
    use crate::value::Len;
    let (ch, ex) = ch_ex_px(family, size_px);
    match len {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size_px,
        Some(Len::Ch(k)) => k * ch,
        Some(Len::Ex(k)) => k * ex,
        Some(Len::Ic(k)) => k * ic_px(family, size_px),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Len;

    #[test]
    fn ic_spacing_uses_the_measured_ideographic_advance() {
        super::super::install_probe(|_, size| (size * 0.25, size * 0.75, size, size * 1.5));
        assert_eq!(spacing_px(Some(Len::Ic(1.0)), "Probe", 20.0), 30.0);
        assert_eq!(spacing_px(Some(Len::Ic(0.5)), "Probe", 40.0), 30.0);
        assert_eq!(spacing_px(Some(Len::Ic(-1.0)), "Probe", 20.0), -30.0);
        assert_eq!(spacing_px(Some(Len::Ch(1.0)), "Probe", 20.0), 5.0);
        super::super::PROBE.with(|p| *p.borrow_mut() = None);
        super::super::CACHE.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn ic_spacing_has_the_specified_em_fallback_without_a_probe() {
        assert_eq!(spacing_px(Some(Len::Ic(1.0)), "Missing", 24.0), 24.0);
    }
}
