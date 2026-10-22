//! Растяжение (stretch-fit) автоматических размеров, ограниченных парами inset-ов якоря.

use crate::style::computed::{Computed, Position};
use crate::style::values::value::{Len, px_lossy_zero};

/// Пара вставок оси, хотя бы одна — `anchor()`, при `auto`-размере: коробка
/// заполняет inset-modified containing block (CSS 2.1 §10.3.7, css-position-3
/// §abspos-auto-size «stretch-fit»). Раскладка видит `anchor()` нулём и
/// растянула бы коробку на весь содержащий блок, поэтому размер берётся из
/// IMCB текущей layout-пробы после публикации её якорей.
pub(super) fn fill_anchor_pairs(
    c: &mut Computed,
    grid_parent: Option<bool>,
    current: Option<(f32, f32)>,
) {
    let anchored = |l: Option<Len>| matches!(l, Some(Len::Anchor(_)));
    let set = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
    let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    // Содержащий блок — ячейка сетки или окно для `fixed`: IMCB от рамки
    // всего блока здесь не годится, раскладка решает сама.
    if c.position == Some(Position::Fixed) || matches!(grid_parent, Some(true)) {
        return;
    }
    let x = auto(c.width)
        && set(c.inset.left)
        && set(c.inset.right)
        && (anchored(c.inset.left) || anchored(c.inset.right));
    let y = auto(c.height)
        && set(c.inset.top)
        && set(c.inset.bottom)
        && (anchored(c.inset.top) || anchored(c.inset.bottom));
    if !x && !y {
        return;
    }
    let Some((w, h)) = current else {
        return;
    };
    let pxv = px_lossy_zero;
    let b = c.borders();
    let inner = c.border_box != Some(true);
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
            c.margin.left,
            c.margin.right,
            b.left,
            b.right,
            c.padding.left,
            c.padding.right,
        );
        c.width = Some(Len::Px((w - e).max(0.0)));
    }
    if y {
        let e = edge(
            c.margin.top,
            c.margin.bottom,
            b.top,
            b.bottom,
            c.padding.top,
            c.padding.bottom,
        );
        c.height = Some(Len::Px((h - e).max(0.0)));
    }
}
