//! Подготовка полноширинных плавающих детей к стопке колонок.

use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn block_floats(
    ge: &Element,
    positioned: impl Fn(&Computed) -> bool,
) -> Option<Element> {
    // Плавающий прямой ребёнок во всю ширину колонки рядом с
    // собой ничего не терпит: строки и блоки встают под ним,
    // как под блоком, — в стопку колонок он идёт БЛОКОМ и
    // рвётся по колонкам вместе с потоком (css-break-3 §4:
    // флоат — фрагментируемая коробка; `css-break/float-001`:
    // флоат 200px в колонках по 100 — прежний путь рисовал его
    // соседом стопки одним куском).
    let zero_m = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    };
    let full_float = |c: &Element| {
        c.style.float.is_some_and(|f| f != 0)
            && matches!(c.style.width, Some(Len::Pct(k)) if (k - 1.0).abs() < 1e-4)
            && zero_m(&c.style.margin.left)
            && zero_m(&c.style.margin.right)
            // Поля флоата не схлопываются и у края колонки не
            // усекаются (CSS 2.1 §8.3.1), а у блока стопки —
            // да: флоат с вертикальными полями — прежним путём
            // (`multicol-fill-balance-037`: `margin: 40px 0`).
            && zero_m(&c.style.margin.top)
            && zero_m(&c.style.margin.bottom)
            && c.style.shape_outside.is_none()
            && !positioned(&c.style)
    };

    ge.children
        .iter()
        .any(|n| matches!(n, Node::Element(c) if full_float(c)))
        .then(|| {
            let mut g = ge.clone();
            for n in g.children.iter_mut() {
                if let Node::Element(c) = n
                    && full_float(c)
                {
                    c.style.float = None;
                    c.style.clear = None;
                    c.attrs.push(("kamin-float-block".into(), "1".into()));
                }
            }
            g
        })
}
