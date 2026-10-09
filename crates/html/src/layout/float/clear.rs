//! `clear` и пропуск флоатов.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::block::containing::CB_WIDTH;
use crate::layout::block::struts::top_edge_open;
use crate::paint::effects::grouped::px_of2;
use crate::render::{in_flow, inline_marked_block, is_blank, own_context};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Отрисовать корневые узлы документа.
///
/// Годится для короткого документа — виджета, ответа модели. Длинный документ
/// рисуйте по блокам (`render_block`): раскладка в GPUI считается заново
/// каждый кадр, поэтому стоимость кадра обязана зависеть от видимой части, а
/// не от размера документа.
/// Есть ли в поддереве хоть одна коробка, которую `clear` может очищать:
/// флоат (свой, у `::first-letter` или под `:hover`) либо буквица
/// (`initial-letter`).
pub(crate) fn has_clearable(nodes: &[Node]) -> bool {
    let floats = |c: &Computed| c.float.is_some_and(|f| f != 0) || c.initial_letter.is_some();
    nodes.iter().any(|n| match n {
        Node::Element(e) => {
            floats(&e.style)
                || e.first_letter.as_ref().is_some_and(floats)
                || e.hover.as_ref().is_some_and(floats)
                || has_clearable(&e.children)
        }
        Node::Text(_) => false,
    })
}

pub(crate) fn has_clear(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Element(e) => e.style.clear.is_some() || has_clear(&e.children),
        Node::Text(_) => false,
    })
}

pub(crate) fn strip_clear(nodes: &mut [Node]) {
    for n in nodes {
        if let Node::Element(e) = n {
            e.style.clear = None;
            e.style.clear_inherit = false;
            if let Some(h) = e.hover.as_mut() {
                h.clear = None;
            }
            strip_clear(&mut e.children);
        }
    }
}

/// CSS 2.1 §9.5.2: clearance вводится только ради флоатов выше по потоку
/// того же контекста. В документе без единого флоата `clear` ничего не
/// значит — и, в частности, не отделяет поля (§8.3.1 говорит о коробке «with
/// clearance», а не о коробке с `clear`). Наши цепи схлопывания судят по
/// самому свойству, поэтому в таком документе оно снимается целиком
/// (`margin-collapse-135`: девять `clear: both` без флоатов — поля обязаны
/// схлопнуться в ноль). Флоаты есть — дерево не трогается.
pub(crate) fn without_inert_clear(nodes: &[Node]) -> Option<Vec<Node>> {
    if has_clearable(nodes) || !has_clear(nodes) {
        return None;
    }
    let mut copy = nodes.to_vec();
    strip_clear(&mut copy);
    Some(copy)
}

/// §9.5, последний абзац: коробка своего контекста, которой РЯДОМ с флоатами
/// заведомо нет места, — «implementations should clear the said element by
/// placing it below any preceding floats». `float_min_w` — нижняя оценка
/// ширины УЗЧАЙШЕГО флоата: на любой полосе с флоатом свободно не больше
/// `cb − float_min_w`. Содержащий блок — `CB_WIDTH`, ближайший предок с
/// шириной в точках: это верхняя оценка настоящей ширины, поэтому «не
/// влезает» при ней не влезает и в настоящую.
pub(crate) fn bfc_no_fit(n: &Element, float_min_w: f32) -> bool {
    let Some(cb) = CB_WIDTH.get() else {
        return false;
    };
    let Some(Len::Px(w)) = n.style.width else {
        return false;
    };
    let b = n.style.borders();
    let bw = if n.style.border_box == Some(true) {
        Some(w)
    } else {
        match (
            px_of2(&n.style.padding.left),
            px_of2(&n.style.padding.right),
            px_of2(&b.left),
            px_of2(&b.right),
        ) {
            (Some(pl), Some(pr), Some(bl), Some(br)) => Some(w + pl + pr + bl + br),
            _ => None,
        }
    };
    !(n.inline && !inline_marked_block(n))
        && in_flow(&n.style)
        && matches!(
            n.style.display,
            None | Some(Display::Block) | Some(Display::Flex) | Some(Display::Grid)
        )
        && own_context(n)
        && bw.is_some_and(|bw| bw + float_min_w > cb + 0.01)
}

/// `clear` первого ребёнка ведущей цепочки ПРОЗРАЧНОЙ обёртки: обычный блок,
/// открытый сверху, без фона, без своего `clear` и позиционирования, а поля
/// цепочки ниже него нулевые. Тогда верх очищающего внука совпадает с верхом
/// обёртки, и спуск ВСЕЙ обёртки под флоаты (§9.5.2) ничего видимого не
/// сдвигает — а ряд обтекания иначе увозит её в колонку сбоку от флоата
/// (`second-float-inside-empty-cleared-block`: второй флоат на x = 100, y = 1
/// вместо x = 0, y = 50).
pub(crate) fn leading_clear(e: &Element) -> Option<i8> {
    if (e.inline && !inline_marked_block(e))
        || !in_flow(&e.style)
        || !matches!(e.style.display, None | Some(Display::Block))
        || !top_edge_open(e)
        || e.style.clear.is_some()
        || e.style.position.is_some()
        || e.style.background.is_some()
        || e.style.bg_image.is_some()
        || e.style.gradient.is_some()
    {
        return None;
    }
    let Node::Element(k) = e.children.iter().find(|n| !is_blank(n))? else {
        return None;
    };
    if (k.inline && !inline_marked_block(k))
        || !in_flow(&k.style)
        || !matches!(k.style.margin.top, None | Some(Len::Px(0.0)))
    {
        return None;
    }
    k.style.clear.or_else(|| leading_clear(k))
}

/// Обрывает ли `clear` обтекание со стороны `side` (-1 слева, 1 справа).
///
/// `clear: left` правый флоат не трогает и наоборот (CSS 2.1 §9.5.2);
/// прежде `clear` был двузначным, и любая сторона обрывала любой ряд.
pub(crate) fn clears_side(clear: Option<i8>, side: i8) -> bool {
    matches!(clear, Some(c) if c == 0 || c == side)
}
