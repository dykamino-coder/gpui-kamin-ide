//! Флоаты: полосы обтекания, очистка, буквица, обтекание по форме.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::float::band_host::px_margin_box;
use crate::render::is_blank;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;

pub mod band_flow_host;
pub mod band_host;
pub mod band_measured;
pub mod band_nest;
pub mod clear;
pub mod float_flow;
pub mod initial_letter;
pub mod shape_flow;
pub mod wrap;
pub(super) mod band_clearance;
mod band_dimensions;
mod float_atom;
mod float_clear_scope;
pub(super) mod inline_floats;
pub(crate) mod shapes;
pub(crate) mod rounded_box;

pub(crate) fn block_like_float(c: &Computed) -> bool {
    c.float.unwrap_or(0) != 0 && matches!(c.width, Some(Len::Pct(p)) if p >= 0.9999)
}

/// Коробка, у которой в потоке НЕТ НИЧЕГО, кроме флоатов — не больше одного
/// на сторону, пустых, без `clear` и позиционирования, с шириной и высотой в
/// точках. Вернуть высоту их общего ряда: наша раскладка ставит такую пару
/// одним флекс-рядом (`wrap_floats`), и ряд высотой `max` — ровно то место,
/// которое флоаты занимают в блочной оси и которое `clear` следующего брата
/// обязан пройти (§9.5.2). Укладке колонок (`shape_full`) этого хватает,
/// чтобы резать коробку по колонкам вместо отказа от всей стопки
/// (`floats-clear-multicol-*`: флоаты 250 в колонках по 100).
pub(super) fn float_only_box(c: &Element) -> Option<f32> {
    if c.inline || !matches!(c.style.display, None | Some(Display::Block)) {
        return None;
    }
    let (mut left, mut right, mut tall) = (0u8, 0u8, 0.0f32);
    for n in &c.children {
        let f = match n {
            Node::Text(t) if blank_text(t) => continue,
            Node::Element(f)
                if f.style.float.is_some_and(|s| s != 0) && !block_like_float(&f.style) =>
            {
                f
            }
            _ => return None,
        };
        if !matches!(f.style.width, Some(Len::Px(_)))
            || !matches!(f.style.height, Some(Len::Px(_)))
            || f.style.clear.is_some()
            || f.style.position.is_some()
            || !f.children.iter().all(is_blank)
        {
            return None;
        }
        if f.style.float.is_some_and(|s| s < 0) {
            left += 1;
        } else {
            right += 1;
        }
        tall = tall.max(px_margin_box(&f.style)?.1);
    }
    (left + right > 0 && left <= 1 && right <= 1).then_some(tall)
}

pub(super) fn has_float(n: &Element, depth: u8) -> bool {
    if depth == 0 {
        return false;
    }
    n.children.iter().any(|k| match k {
        Node::Element(e) => {
            (e.style.float.unwrap_or(0) != 0 && !block_like_float(&e.style))
                || has_float(e, depth - 1)
        }
        _ => false,
    })
}
