//! Подготовка полноширинных плавающих детей к стопке колонок.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::tolerance::COORD_EPS;

pub(crate) fn block_floats(
    ge: &Element,
    positioned: impl Fn(&Computed) -> bool,
    col_w: Option<f32>,
) -> Option<Element> {
    floats_in(ge, &positioned, col_w)
}

fn floats_in(
    ge: &Element,
    positioned: &dyn Fn(&Computed) -> bool,
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
        Some(Len::Px(v)) => v.abs() < COORD_EPS,
        _ => false,
    };
    // Во всю ширину — `width: 100%` или точечная ширина не уже колонки.
    // Второе — только ПОСЛЕДНЕМУ содержательному ребёнку: блок-сосед
    // после флоата встал бы не под ним, а рядом по вертикали (его коробка
    // лежит под флоатом, CSS 2.1 §9.5), а стопка поставила бы его ниже
    // (`spanner-in-child-after-parallel-flow-001`: флоат 50px в колонке
    // 50px, дальше только пустой обломок предка спаннера).
    let wide_px = |c: &Element| matches!((c.style.computed().width, col_w), (Some(Len::Px(w)), Some(cw)) if cw > 0.0 && w >= cw - COORD_EPS);
    let inert = |n: &Node| match n {
        Node::Text(_) => is_blank(n),
        Node::Element(k) => {
            k.children.iter().all(is_blank)
                && matches!(k.style.computed().height, None | Some(Len::Auto))
                && matches!(k.style.computed().min_height, None | Some(Len::Auto))
                && k.style.computed().padding.top.is_none()
                && k.style.computed().padding.bottom.is_none()
                && k.style.computed().borders().top.is_none()
                && k.style.computed().borders().bottom.is_none()
                && zero_m(&k.style.computed().margin.top)
                && zero_m(&k.style.computed().margin.bottom)
        }
    };
    let last_content = |i: usize| {
        ge.children
            .get(i + 1..)
            .is_some_and(|rest| rest.iter().all(inert))
    };
    let full_float = |i: usize, c: &Element| {
        c.style.computed().float.is_some_and(|f| f != 0)
            && (matches!(c.style.computed().width, Some(Len::Pct(k)) if (k - 1.0).abs() < 1e-4)
                || (wide_px(c) && last_content(i)))
            && zero_m(&c.style.computed().margin.left)
            && zero_m(&c.style.computed().margin.right)
            // Поля флоата не схлопываются и у края колонки не
            // усекаются (CSS 2.1 §8.3.1), а у блока стопки —
            // да: флоат с вертикальными полями — прежним путём
            // (`multicol-fill-balance-037`: `margin: 40px 0`).
            && zero_m(&c.style.computed().margin.top)
            && zero_m(&c.style.computed().margin.bottom)
            && c.style.computed().shape_outside.is_none()
            && !positioned(&c.style.computed())
    };
    let full: Vec<bool> = ge
        .children
        .iter()
        .enumerate()
        .map(|(i, n)| matches!(n, Node::Element(c) if full_float(i, c)))
        .collect();
    // Простая блочная обёртка (без высоты, отбивок, рамок, полей и
    // позиционирования) сама в колонку не режется иначе, чем её дети:
    // флоат во всю ширину внутри неё — тот же случай, что у прямого
    // ребёнка (`float-in-self-collapsing-block-000`: обёртка из одного
    // флоата 100% рисовала его одним куском поверх разрыва колонки,
    // а замер стопки — `block_like_float` в `kid_shapes` — уже резал).
    let nested: Vec<Option<Element>> = ge
        .children
        .iter()
        .map(|n| match n {
            Node::Element(c) if plain_wrapper(c, positioned) => floats_in(c, positioned, col_w),
            _ => None,
        })
        .collect();
    (full.iter().any(|f| *f) || nested.iter().any(Option::is_some)).then(|| {
        let mut g = ge.clone();
        for ((n, f), w) in g.children.iter_mut().zip(&full).zip(nested) {
            if let Node::Element(c) = n {
                if *f {
                    c.style.used_mut().float = None;
                    c.style.used_mut().clear = None;
                    c.marks.float_block = true;
                } else if let Some(w) = w {
                    *c = w;
                }
            }
        }
        g
    })
}

/// Блок потока, прозрачный для разреза колонок: ни высоты, ни отбивок,
/// рамок и вертикальных полей, ни флоата и позиционирования, ни нового
/// контекста форматирования (`overflow`/`display`).
fn plain_wrapper(c: &Element, positioned: &dyn Fn(&Computed) -> bool) -> bool {
    let s = &c.style.computed();
    let none = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < COORD_EPS,
        _ => false,
    };
    let b = s.borders();
    !c.inline
        && matches!(
            s.display,
            None | Some(crate::style::computed::Display::Block)
        )
        && s.float.unwrap_or(0) == 0
        && s.clear.is_none()
        && !positioned(s)
        && s.position.is_none()
        && matches!(s.height, None | Some(Len::Auto))
        && s.min_height.is_none()
        && s.max_height.is_none()
        && crate::layout::page::paged::visible_overflow(s)
        && [
            &s.margin.top,
            &s.margin.bottom,
            &s.padding.top,
            &s.padding.bottom,
            &s.padding.left,
            &s.padding.right,
            &b.top,
            &b.bottom,
            &b.left,
            &b.right,
        ]
        .into_iter()
        .all(none)
}
