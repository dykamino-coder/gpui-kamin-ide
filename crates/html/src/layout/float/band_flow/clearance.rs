//! Верх коробки с `clear` в плане измеряемого хоста, когда её верхнее поле примыкает к
//! предшествующим флоатам (CSS 2.1 §9.5.2): гипотетическое место при `clear: none` против
//! низа флоатов названной стороны; клиренс может выйти отрицательным.

use crate::layout::float::bands::FloatBands;

/// CSS 2.1 §9.5.2: test the hypothetical position with `clear: none`.
/// An adjoining start margin would move both the block and the preceding
/// floats. Once clearance separates that margin, place the border edge at
/// the floats' bottom; the clearance may consequently be negative.
/// Blink block_layout_algorithm.cc:1883-1908 likewise distinguishes adjoining
/// and non-adjoining offsets before resolving the BFC position.
pub(super) fn top(
    bands: &FloatBands,
    clear: Option<i8>,
    y: f32,
    margin: f32,
    adjoining: bool,
) -> f32 {
    let cleared = bands.clearance_including_initial_letter(clear, y);
    if adjoining && cleared > y {
        cleared
    } else {
        bands.clearance(clear, y + margin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continued_float_clearance_uses_its_remaining_margin_edge() {
        let mut bands = FloatBands::new(120.);
        bands.resume_float(-1, 60., 100., -40., 0.);
        assert_eq!(top(&bands, Some(-1), 10., -30., false), 60.);
        assert_eq!(top(&bands, Some(-1), 10., 50., true), 60.);
        // CSS2 §9.5.2: отделение примыкающего поля даёт отрицательный
        // clearance, а поле ниже float без примыкания сохраняется.
        assert_eq!(top(&bands, Some(-1), 10., 200., true), 60.);
        assert_eq!(top(&bands, Some(-1), 10., 200., false), 210.);
        assert_eq!(top(&bands, Some(1), 10., -30., false), -20.);
    }
}
