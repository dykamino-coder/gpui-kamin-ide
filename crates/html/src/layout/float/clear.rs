//! Предикаты `clear` для раскладки флоатов (CSS 2.1 §9.5.2): снятие `clear` в дереве без
//! единого флоата (`without_inert_clear` — клиренса там нет, и поля обязаны схлопнуться),
//! коробка своего контекста, которой заведомо нет места рядом с флоатами (`bfc_no_fit`,
//! §9.5 последний абзац).

use crate::dom::{Element, Node};
use crate::layout::box_kind::{in_flow, inline_marked_block, own_context};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Len, px_missing_zero};

/// Есть ли в поддереве хоть одна коробка, которую `clear` может очищать:
/// флоат (свой, у `::first-letter` или под `:hover`) либо буквица
/// (`initial-letter`).
fn has_clearable(nodes: &[Node]) -> bool {
    let floats = |c: &Computed| c.float.is_some_and(|f| f != 0) || c.initial_letter.is_some();
    nodes.iter().any(|n| match n {
        Node::Element(e) => {
            floats(&e.style.used())
                || e.first_letter
                    .as_ref()
                    .is_some_and(|l| floats(l.cascaded()))
                || e.hover.as_ref().is_some_and(|l| floats(l.cascaded()))
                || has_clearable(&e.children)
        }
        Node::Text(_) => false,
    })
}

fn has_clear(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Element(e) => e.style.used().clear.is_some() || has_clear(&e.children),
        Node::Text(_) => false,
    })
}

fn strip_clear(nodes: &mut [Node]) {
    for n in nodes {
        if let Node::Element(e) = n {
            e.style.used_mut().clear = None;
            e.style.used_mut().clear_inherit = false;
            if let Some(h) = e.hover.as_mut() {
                h.used_mut().clear = None;
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
/// `cb − float_min_w`. Содержащий блок `cb` — ширина ближайшего предка в
/// точках (`LayoutCx::cb_width`): это верхняя оценка настоящей ширины,
/// поэтому «не влезает» при ней не влезает и в настоящую.
pub(crate) fn bfc_no_fit(n: &Element, float_min_w: f32, cb: Option<f32>) -> bool {
    let Some(cb) = cb else {
        return false;
    };
    let Some(Len::Px(w)) = n.style.used().width else {
        return false;
    };
    let b = n.style.used().borders();
    let bw = if n.style.used().border_box == Some(true) {
        Some(w)
    } else {
        match (
            px_missing_zero(n.style.used().padding.left),
            px_missing_zero(n.style.used().padding.right),
            px_missing_zero(b.left),
            px_missing_zero(b.right),
        ) {
            (Some(pl), Some(pr), Some(bl), Some(br)) => Some(w + pl + pr + bl + br),
            _ => None,
        }
    };
    (!n.inline || inline_marked_block(n))
        && in_flow(&n.style.used())
        && matches!(
            n.style.used().display,
            None | Some(Display::Block) | Some(Display::Flex) | Some(Display::Grid)
        )
        && own_context(n)
        && bw.is_some_and(|bw| bw + float_min_w > cb + 0.01)
}
