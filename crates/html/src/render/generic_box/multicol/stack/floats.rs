//! Подготовка полноширинных плавающих детей к стопке колонок.

use crate::dom::{Element, Node};
use crate::render::is_blank;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn block_floats(
    ge: &Element,
    positioned: impl Fn(&Computed) -> bool,
    col_w: Option<f32>,
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
    // Во всю ширину — `width: 100%` или точечная ширина не уже колонки.
    // Второе — только ПОСЛЕДНЕМУ содержательному ребёнку: блок-сосед
    // после флоата встал бы не под ним, а рядом по вертикали (его коробка
    // лежит под флоатом, CSS 2.1 §9.5), а стопка поставила бы его ниже
    // (`spanner-in-child-after-parallel-flow-001`: флоат 50px в колонке
    // 50px, дальше только пустой обломок предка спаннера).
    let wide_px = |c: &Element| matches!((c.style.width, col_w), (Some(Len::Px(w)), Some(cw)) if cw > 0.0 && w >= cw - 0.01);
    let inert = |n: &Node| match n {
        Node::Text(_) => is_blank(n),
        Node::Element(k) => {
            k.children.iter().all(is_blank)
                && matches!(k.style.height, None | Some(Len::Auto))
                && matches!(k.style.min_height, None | Some(Len::Auto))
                && k.style.padding.top.is_none()
                && k.style.padding.bottom.is_none()
                && k.style.border_width.top.is_none()
                && k.style.border_width.bottom.is_none()
                && zero_m(&k.style.margin.top)
                && zero_m(&k.style.margin.bottom)
        }
    };
    let last_content = |i: usize| {
        ge.children
            .get(i + 1..)
            .is_some_and(|rest| rest.iter().all(inert))
    };
    let full_float = |i: usize, c: &Element| {
        c.style.float.is_some_and(|f| f != 0)
            && (matches!(c.style.width, Some(Len::Pct(k)) if (k - 1.0).abs() < 1e-4)
                || (wide_px(c) && last_content(i)))
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
    let full: Vec<bool> = ge
        .children
        .iter()
        .enumerate()
        .map(|(i, n)| matches!(n, Node::Element(c) if full_float(i, c)))
        .collect();
    full.iter().any(|f| *f).then(|| {
        let mut g = ge.clone();
        for (n, f) in g.children.iter_mut().zip(&full) {
            if let Node::Element(c) = n
                && *f
            {
                c.style.float = None;
                c.style.clear = None;
                c.attrs.push(("kamin-float-block".into(), "1".into()));
            }
        }
        g
    })
}
