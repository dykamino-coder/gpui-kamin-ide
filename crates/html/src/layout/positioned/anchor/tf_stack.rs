//! Стек трансформаций якорей: границы содержащего блока и отображение точек.

use super::CB;
use crate::layout::positioned::anchor::TF;
use crate::layout::positioned::anchor::TF_NEXT;
use gpui::{Bounds, Pixels, px};

/// Read the actual padding box of an absolute containing block in this frame.
pub(crate) fn containing_bounds(node: u64) -> Option<Bounds<Pixels>> {
    CB.with(|map| map.borrow().get(&node).copied())
}

/// Положить плоский трансформ на стек подготовки (`Transformed::prepaint`).
pub fn tf_push(m: [[f32; 3]; 2]) {
    let id = TF_NEXT.with(|n| {
        let v = n.get() + 1;
        n.set(v);
        v
    });
    TF.with(|s| s.borrow_mut().push((id, m)));
}

/// Снять трансформ со стека подготовки.
pub fn tf_pop() {
    TF.with(|s| {
        s.borrow_mut().pop();
    });
}

/// Рамка через весь стек: углы идут от ВНУТРЕННЕГО трансформа к внешнему
/// (экран = внешний(…внутренний(p))), результат — объемлющий прямоугольник
/// (§2: «axis-aligned bounding rectangle»). Второе — номер внутреннего
/// трансформа (0 — стек пуст).
pub(super) fn tf_map(r: Bounds<Pixels>) -> (Bounds<Pixels>, u32) {
    TF.with(|s| {
        let s = s.borrow();
        let Some(&(top, _)) = s.last() else {
            return (r, 0);
        };
        let x0 = f32::from(r.origin.x);
        let y0 = f32::from(r.origin.y);
        let x1 = x0 + f32::from(r.size.width);
        let y1 = y0 + f32::from(r.size.height);
        let (mut lx, mut ly, mut hx, mut hy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (mut x, mut y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
            for (_, m) in s.iter().rev() {
                (x, y) = (
                    m[0][0] * x + m[0][1] * y + m[0][2],
                    m[1][0] * x + m[1][1] * y + m[1][2],
                );
            }
            lx = lx.min(x);
            ly = ly.min(y);
            hx = hx.max(x);
            hy = hy.max(y);
        }
        let b = Bounds {
            origin: gpui::point(px(lx), px(ly)),
            size: gpui::size(px(hx - lx), px(hy - ly)),
        };
        (b, top)
    })
}

/// Лежит ли трансформ `id` на текущем стеке — то есть предок ли он коробки,
/// которая сейчас готовится.
pub(super) fn tf_under(id: u32) -> bool {
    TF.with(|s| s.borrow().iter().any(|(i, _)| *i == id))
}
