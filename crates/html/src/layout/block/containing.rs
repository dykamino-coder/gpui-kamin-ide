//! Ширина содержимого коробки как содержащий блок её детей на спуске схлопывания полей.
// owner: A

use crate::layout::block::margins::{COLLAPSE_CB_WIDTH_PX, COLLAPSE_FONT_PX};
use crate::render::in_flow;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Ширина СОДЕРЖИМОГО коробки в точках — содержащий блок её детей (CSS 2.1
/// §10.1 п.2) — для процентных полей внуков в цепочках схлопывания. `cb` —
/// содержащий блок самой коробки; `None` — в точках не выводится.
fn inner_width_px(c: &Computed, cb: Option<f32>) -> Option<f32> {
    let side = |l: Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        Some(Len::Pct(k)) => cb.map(|w| k * w),
        _ => None,
    };
    let b = c.borders();
    let edges =
        || Some(side(c.padding.left)? + side(c.padding.right)? + side(b.left)? + side(b.right)?);
    match c.width {
        Some(Len::Px(w)) if c.border_box == Some(true) => Some((w - edges()?).max(0.0)),
        Some(Len::Px(w)) => Some(w),
        Some(Len::Pct(k)) if c.border_box != Some(true) => cb.map(|w| k * w),
        // §10.3.3: блок в потоке с `width: auto` занимает содержащий блок за
        // вычетом своих полей, рамок и отступов (auto-поле — ноль).
        None | Some(Len::Auto) if in_flow(c) => {
            Some((cb? - side(c.margin.left)? - side(c.margin.right)? - edges()?).max(0.0))
        }
        _ => None,
    }
}

/// `f` под шириной содержимого коробки `c` вместо ширины уровня. Прежде
/// цепочка мерила доли внуков от содержащего блока УРОВНЯ: у `body` с
/// `width: auto` его нет, и `15%` у `#parent` внутри `#grand-parent {width:
/// 400px}` роняло цепочку целиком — поля абзаца и `#parent` складывались
/// (16 + 60 вместо 60, `margin-percentage-inherit-001`).
pub(super) fn with_inner_cb<T>(c: &Computed, f: impl FnOnce() -> T) -> T {
    let level = COLLAPSE_CB_WIDTH_PX.with(std::cell::Cell::get);
    let prev = COLLAPSE_CB_WIDTH_PX.with(|w| w.replace(inner_width_px(c, level)));
    // Кегль уровня — тоже от спуска: поля детей в `em` без своего кегля (и
    // с кеглем в `em`) меряются от кегля ЭТОЙ коробки, а не уровня выше.
    // Прежде `html{font-size:2em} p{font-size:.5em}` схлопывал поле абзаца
    // сквозь `body` по 8 точкам вместо 16 (`numbers-units-021`).
    let font = COLLAPSE_FONT_PX.with(std::cell::Cell::get);
    let own = match c.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * font,
        _ => font,
    };
    let prev_font = COLLAPSE_FONT_PX.with(|w| w.replace(own));
    let out = f();
    COLLAPSE_CB_WIDTH_PX.with(|w| w.set(prev));
    COLLAPSE_FONT_PX.with(|w| w.set(prev_font));
    out
}
