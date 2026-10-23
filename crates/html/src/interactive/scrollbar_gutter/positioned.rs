//! Gutter полос уменьшает содержащий блок абсолютных и фиксированных коробок (CSS Overflow 3
//! §4.2): их inset'ы и размеры в долях пересчитываются от содержащего блока без gutter.

use crate::document::state::ValueCx;
use crate::style::computed::{BoxStyle, Position};
use gpui::{DefiniteLength, Div, Length, Styled};
fn adjust(value: &mut Option<Length>, side: f32, total: f32, vcx: ValueCx<'_>) {
    let Some(Length::Definite(v)) = value else {
        return;
    };
    let fraction = match v {
        DefiniteLength::Fraction(k) | DefiniteLength::Calc(_, k) => *k,
        _ => 0.0,
    };
    *v = super::add(*v, side - fraction * total, vcx);
}
pub(crate) fn apply(mut div: Div, c: &BoxStyle, vcx: ValueCx<'_>) -> Div {
    if !matches!(c.position, Some(Position::Absolute | Position::Fixed)) {
        return div;
    }
    let (used, native) =
        if c.cb_node == 0 || c.position == Some(Position::Fixed) && !c.transform_ancestor {
            (c.icb_scroll_insets.unwrap_or_default(), [0.0; 4])
        } else {
            c.cb_scroll_insets
        };
    let delta: [f32; 4] = std::array::from_fn(|side| used[side] - native[side]);
    if delta == [0.0; 4] {
        return div;
    }
    let totals = [delta[0] + delta[2], delta[1] + delta[3]];
    let style = div.style();
    adjust(&mut style.inset.left, delta[0], totals[0], vcx);
    adjust(&mut style.inset.right, delta[2], totals[0], vcx);
    adjust(&mut style.inset.top, delta[1], totals[1], vcx);
    adjust(&mut style.inset.bottom, delta[3], totals[1], vcx);
    for value in [
        &mut style.size.width,
        &mut style.min_size.width,
        &mut style.max_size.width,
    ] {
        adjust(value, 0.0, totals[0], vcx);
    }
    for value in [
        &mut style.size.height,
        &mut style.min_size.height,
        &mut style.max_size.height,
    ] {
        adjust(value, 0.0, totals[1], vcx);
    }
    div
}
