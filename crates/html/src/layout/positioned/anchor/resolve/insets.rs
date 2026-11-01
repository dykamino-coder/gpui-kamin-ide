//! Растяжение (stretch-fit) автоматических размеров, ограниченных парами inset-ов якоря.

use crate::style::computed::BoxStyle;
use crate::style::computed::Position;
use crate::style::values::value::{Len, px_lossy_zero};

/// Пара вставок оси, хотя бы одна — `anchor()`, при `auto`-размере: коробка
/// заполняет inset-modified containing block (CSS 2.1 §10.3.7, css-position-3
/// §abspos-auto-size «stretch-fit»). Раскладка видит `anchor()` нулём и
/// растянула бы коробку на весь содержащий блок, поэтому размер берётся из
/// IMCB текущей layout-пробы после публикации её якорей.
pub(super) fn fill_anchor_pairs(
    c: &mut BoxStyle,
    grid_parent: Option<bool>,
    current: Option<(f32, f32)>,
) {
    let anchored = |l: Option<Len>| matches!(l, Some(Len::Anchor(_)));
    let set = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
    let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    // Содержащий блок — ячейка сетки или окно для `fixed`: IMCB от рамки
    // всего блока здесь не годится, раскладка решает сама.
    if c.reset_positioned.position == Some(Position::Fixed) || matches!(grid_parent, Some(true)) {
        return;
    }
    let x = auto(c.reset_box_layout.width)
        && set(c.reset_positioned.inset.left)
        && set(c.reset_positioned.inset.right)
        && (anchored(c.reset_positioned.inset.left) || anchored(c.reset_positioned.inset.right));
    let y = auto(c.reset_box_layout.height)
        && set(c.reset_positioned.inset.top)
        && set(c.reset_positioned.inset.bottom)
        && (anchored(c.reset_positioned.inset.top) || anchored(c.reset_positioned.inset.bottom));
    if !x && !y {
        return;
    }
    let Some((w, h)) = current else {
        return;
    };
    let pxv = px_lossy_zero;
    let b = c.borders();
    let inner = c.reset_border.border_box != Some(true);
    let edge = |m0: Option<Len>, m1: Option<Len>, b0, b1, p0, p1| {
        pxv(m0)
            + pxv(m1)
            + if inner {
                pxv(b0) + pxv(b1) + pxv(p0) + pxv(p1)
            } else {
                0.0
            }
    };
    if x {
        let e = edge(
            c.reset_box_layout.margin.left,
            c.reset_box_layout.margin.right,
            b.left,
            b.right,
            c.reset_box_layout.padding.left,
            c.reset_box_layout.padding.right,
        );
        c.reset_box_layout.width = Some(Len::Px((w - e).max(0.0)));
    }
    if y {
        let e = edge(
            c.reset_box_layout.margin.top,
            c.reset_box_layout.margin.bottom,
            b.top,
            b.bottom,
            c.reset_box_layout.padding.top,
            c.reset_box_layout.padding.bottom,
        );
        c.reset_box_layout.height = Some(Len::Px((h - e).max(0.0)));
    }
}
