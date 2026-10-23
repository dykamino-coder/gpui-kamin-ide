//! Самовыравнивание в прямоугольнике статической позиции.

use crate::dom::Element;
use crate::style::computed::{Align, Computed};

/// Доли выравнивания абсолюта в прямоугольнике статической позиции
/// (`containing_block::spot`, поле `self_align`). У блочного родителя прямоугольник по
/// строчной оси — края его содержимого, по блочной — нулевой, в самой
/// статической точке (css-position-3 §static-position-rectangle), и
/// `justify-self`/`align-self` выравнивают коробку в нём (css-align-3
/// §justify-abspos, §align-abspos; эталон `align-self-static-position-001`:
/// `end` — `top: -45px; left: 30px`, низ коробки в точке).
///
/// Только горизонтальное письмо родителя и блочный абсолют: у строчного
/// родителя и в вертикальном письме прямоугольник другой
/// (`align-self-static-position-002/003/004/007`) — `None`, прежний ход.
/// `None` и тогда, когда ни одна ось не задала своего выравнивания.
/// `normal`/`stretch`/`first baseline` у абсолюта ведут себя как `start`,
/// `last baseline` — как `end` (css-align-3 §9.3); `anchor-center` без
/// якоря — как `center` (css-anchor-position-1 §anchor-center), с якорем его
/// ведёт `anchor::place::place_one`, и сюда он не берётся.
pub(crate) fn static_self_align(e: &Element, inherited: &Computed) -> Option<(f32, f32)> {
    static_self_align_style(e.style.cascaded(), inherited)
}

pub(crate) fn static_self_align_style(own: &Computed, inherited: &Computed) -> Option<(f32, f32)> {
    if own.inline_display == Some(true) {
        return None;
    }
    let anchorless = own.position_anchor.is_none()
        && own.position_area.is_none()
        && own.implicit_anchor.is_none();
    let frac = |a: Option<Align>, last: bool| match a {
        Some(Align::Center) => Some(0.5),
        Some(Align::AnchorCenter) if anchorless => Some(0.5),
        Some(Align::End) => Some(1.0),
        Some(Align::Baseline) if last => Some(1.0),
        Some(Align::Start) => Some(0.0),
        _ => None,
    };
    // `justify-self: auto` takes the parent's `justify-items` (css-align-3
    // §6.1; Blink `InlineStaticPositionEdge` with the block parent's style
    // as `justify_items_style`); `align-items` does not apply in block
    // layout, so `align-self: auto` stays `normal`
    // (`align-items-static-position-001`).
    let jx = match own.justify_self {
        None if !own.justify_self_normal => {
            frac(inherited.justify_items, inherited.justify_items_last)
        }
        value => frac(value, own.justify_self_last),
    };
    let ky = frac(own.align_self, own.align_self_last);
    if jx.is_none() && ky.is_none() {
        return None;
    }
    // Строчная ось: `start`/`end` — по письму родителя, `self-*` — по своему
    // (свой `direction` на этом шаге ещё не унаследован — берём родительский).
    let rtl = if own.justify_self_own_axis {
        own.rtl.or(inherited.rtl)
    } else {
        inherited.rtl
    } == Some(true);
    let kx = match jx {
        Some(k) if rtl => 1.0 - k,
        Some(k) => k,
        None if rtl => 1.0,
        None => 0.0,
    };
    let kb = ky.unwrap_or(0.0);
    // Vertical parent: the inline axis is vertical, the block axis horizontal
    // and its start is the right edge in `vertical-rl`. The fractions stay
    // PHYSICAL (x, y): the static-position rectangle is the parent's content
    // box in the inline axis and zero-thickness at the static point in the
    // block axis, whatever the writing mode (Blink
    // `BlockLayoutAlgorithm::HandleOutOfFlowPositioned` works in logical
    // offsets; `align-self-static-position-002`).
    if inherited.wm().vertical() {
        let kb = if inherited.wm().block_rl() {
            1.0 - kb
        } else {
            kb
        };
        return Some((kb, kx));
    }
    Some((kx, kb))
}

/// Self-alignment fractions `(x, y)` of a block-level absolute box whose
/// static position lies in a line of an INLINE formatting context: the
/// static-position rectangle is zero-thick along the line at the static point
/// and spans the line box across it (css-position-3 §staticpos-rect; Blink
/// `FragmentBuilder::AddOutOfFlowInlineChildCandidate`, which ignores
/// `justify-items`/`align-items`). Horizontal ltr only — other modes keep the
/// previous placement (`None`), as does `normal`/`auto` in both axes.
pub(crate) fn inline_static_self_align(e: &Element, inherited: &Computed) -> Option<(f32, f32)> {
    if inherited.wm().vertical()
        || inherited.wm().turned()
        || inherited.wm().rtl()
        || e.style.cascaded().inline_display == Some(true)
    {
        return None;
    }
    let anchorless = e.style.cascaded().position_anchor.is_none()
        && e.style.cascaded().position_area.is_none()
        && e.style.cascaded().implicit_anchor.is_none();
    let frac = |a: Option<Align>, last: bool| match a {
        Some(Align::Center) => Some(0.5),
        Some(Align::AnchorCenter) if anchorless => Some(0.5),
        Some(Align::End) => Some(1.0),
        Some(Align::Baseline) if last => Some(1.0),
        Some(Align::Start) => Some(0.0),
        _ => None,
    };
    let jx = frac(e.style.cascaded().justify_self, e.style.cascaded().justify_self_last);
    let ky = frac(e.style.cascaded().align_self, e.style.cascaded().align_self_last);
    if jx.is_none() && ky.is_none() {
        return None;
    }
    Some((jx.unwrap_or(0.0), ky.unwrap_or(0.0)))
}
