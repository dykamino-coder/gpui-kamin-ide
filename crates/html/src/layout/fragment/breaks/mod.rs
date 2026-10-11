//! Принудительные разрывы и запреты.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::fragment_size::shape_full;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::Display;
use crate::style::values::value::Len;
mod edges;
pub(crate) use edges::edge_avoid;
pub(crate) use edges::edge_break;

/// Досягаемость внепоточного корня стопки страниц: низ его коробки, а при
/// видимом переполнении — низ стопки его блочных детей (Blink копит
/// переполнение монолита в `BlockBreakToken::monolithic_overflow_` и
/// добавляет страницы, пока оно не кончится, crbug 1402540;
/// `monolithic-overflow-027`: абсолют `contain:size` 4in с ребёнком 8in —
/// «four green pages»). Обрезка `overflow-y` переполнение гасит (`-028`).
pub(crate) fn oof_reach(e: &Element, cx: ShapeCx) -> f32 {
    // `vh`/`vw` — от page area; `bottom: -200vh` тянет низ коробки на две
    // area ниже листа (эталоны `fixedpos-001..009`: копии `bottom: -N00vh` и
    // `top: N00vh` — досягаемость была нулевой, лист один).
    let len = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => Some(*v),
        Some(Len::Vh(k)) => cx.viewport.map(|v| *k * v.1),
        Some(Len::Vw(k)) => cx.viewport.map(|v| *k * v.0),
        _ => None,
    };
    let top = len(&e.style.inset.top).unwrap_or(0.0);
    let from_bottom = len(&e.style.inset.bottom)
        .and_then(|b| cx.viewport.map(|v| v.1 - b))
        .unwrap_or(0.0);
    // Мера `None` (строчное содержимое) — не «пусто»: хотя бы точка высоты,
    // чтобы лист под самой строкой родился (`fixedpos-005-print`: `top:
    // 300vh` внутри `top: 100vh` — текст ровно на краю четвёртого листа).
    let own = shape_full(e, 4, cx).map(|s| s.0).unwrap_or_else(|| {
        if e.children.iter().all(is_blank) {
            0.0
        } else {
            1.0
        }
    });
    let clipped = matches!(
        e.style.overflow_y,
        Some(crate::style::computed::Overflow::Hidden)
            | Some(crate::style::computed::Overflow::Clip)
            | Some(crate::style::computed::Overflow::Scroll)
    );
    let inner: f32 = if clipped {
        0.0
    } else {
        e.children
            .iter()
            .filter_map(|n| match n {
                Node::Element(k) if !k.inline && !out_of_flow(&k.style) => {
                    shape_full(k, 3, cx).map(|s| s.0 + s.1 + s.2)
                }
                _ => None,
            })
            .sum()
    };
    // Абсолютные потомки-абсолюты: содержащий блок — эта коробка, их `top`
    // — от её верха (CSS 2.1 §10.6.4). Мера `shape_full` у коробки со
    // строчным содержимым `None`, и дотяг вложенного `top: 300vh` терялся
    // (`fixedpos-005-print`: три листа вместо пяти).
    let nested = if clipped {
        0.0
    } else {
        e.children
            .iter()
            .filter_map(|n| match n {
                Node::Element(k)
                    if k.style.position == Some(crate::style::computed::Position::Absolute) =>
                {
                    Some(oof_reach(k, cx))
                }
                _ => None,
            })
            .fold(0.0f32, f32::max)
    };
    (top + own.max(inner).max(nested)).max(from_bottom)
}

/// Монолит стопки страниц (css-break-4 §4.1; Blink `IsMonolithic`):
/// замещаемый, прокручиваемый, `break-inside: avoid`, `contain: size`,
/// атомарный строчный. Сплошной строчный набор монолитом НЕ считается:
/// страница режет его по краю, а обе стороны пары режутся одинаково.
pub(crate) fn page_monolith(e: &Element) -> bool {
    let scrolls = |o: Option<crate::style::computed::Overflow>| {
        matches!(o, Some(crate::style::computed::Overflow::Scroll))
    };
    e.style.break_inside_avoid
        || e.style.contain_size == Some(true)
        || scrolls(e.style.overflow_x)
        || scrolls(e.style.overflow_y)
        || matches!(
            e.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        || matches!(
            e.style.display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
}
