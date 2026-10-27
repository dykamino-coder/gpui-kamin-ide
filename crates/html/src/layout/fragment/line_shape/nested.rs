//! Вложенные ряды: допустимость native-раскладки и ширина коробки в колонке.

use super::nested_kid::oof_in_plain_kids;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::box_kind::out_of_flow;
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::positioned::predicates::carries_abspos;
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// Вложенный многоколоночник, который внешняя стопка ведёт РЯДАМИ (`nest_row`):
/// обычный блок с колонками, высотой в точках, без своих рядов,
/// внепоточных и вертикального письма.
pub(crate) fn nested_rows_box(c: &Element) -> bool {
    let s = &c.style.used();
    let sizes = BoxDimensions::for_element(c, s);
    multicol_container(s)
        && (s.column_count.is_some_and(|n| n > 1) || s.column_width.is_some())
        && s.column_height.is_none()
        && s.column_wrap.is_none()
        && (matches!(s.height, Some(Len::Px(h)) if h > 0.0)
            || (matches!(s.height, None | Some(Len::Auto))
                && sizes.minimum[1].is_none()
                && sizes.maximum[1].is_none()))
        && matches!(s.display, None | Some(Display::Block))
        && !s.wm().vertical()
        && s.float.unwrap_or(0) == 0
        && (!carries_abspos(c, 4) || oof_in_plain_kids(c))
        // Ряды считаются от верха СОДЕРЖИМОГО: блочные рамка и отбивка (и их
        // повтор у `box-decoration-break: clone`) сдвинули бы границы рядов с
        // границ внешних колонок (`box-decoration-break-clone-010`).
        && !s.bdb_clone
        && {
            // Нижние рамка и отбивка у `height: auto` допустимы: они встают
            // после последнего ряда (`multicol-breaking-006`), а у заданной
            // высоты сдвинули бы последний ряд.
            let b = s.borders();
            let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
            let auto = matches!(s.height, None | Some(Len::Auto));
            zero(&s.padding.top)
                && zero(&b.top)
                && (auto || (zero(&s.padding.bottom) && zero(&b.bottom)))
        }
        && c.children.iter().any(|n| !is_blank(n))
        // Внепоточный потомок — содержащий блок и дотяг рядами не выражены
        // (`out-of-flow-in-multicolumn-019`).
        && (!oof_descendant(c) || oof_in_plain_kids(c))
}

pub(super) fn oof_descendant(e: &Element) -> bool {
    e.children
        .iter()
        .any(|n| matches!(n, Node::Element(k) if out_of_flow(&k.style.used()) || oof_descendant(k)))
}

/// Ширина коробки (`width` по её `box-sizing`) ребёнка в колонке `cw`.
pub(crate) fn nested_box_w(c: &Element, cw: f32) -> Option<f32> {
    let s = &c.style.used();
    let sizes = BoxDimensions::for_element(c, s);
    let px = |l: &Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let b = s.borders();
    let outer = cw - px(&s.margin.left)? - px(&s.margin.right)?;
    let edges = px(&s.padding.left)? + px(&s.padding.right)? + px(&b.left)? + px(&b.right)?;
    Some(
        if sizes.border_box {
            outer
        } else {
            outer - edges
        }
        .max(0.0),
    )
}
