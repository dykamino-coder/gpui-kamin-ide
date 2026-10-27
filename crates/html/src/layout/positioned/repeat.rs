//! Внепоточные потомки повторяемых секций таблицы (css-tables-3 §repeated-headers): копия
//! шапки/подвала несёт только оформление, и абсолют или `fixed`, чей содержащий блок лежит
//! вне копии, из неё удаляется; здесь же признак коробки — содержащего блока `fixed` и
//! normal-копия печатной группы без внепоточных потомков (их рисует native-источник листов).

use crate::dom::Node;

pub(crate) fn fixed_cb_box(c: &crate::style::computed::Computed) -> bool {
    c.transform.is_some()
        || c.contain_layout == Some(true)
        || c.contain_paint == Some(true)
        || crate::style::computed::filter_contains_fixed(c)
        || c.will_change & crate::style::computed::wc::CB_FIXED != 0
}

/// Повтор шапки/подвала таблицы (css-tables-3 §repeated-headers) — копия
/// ОФОРМЛЕНИЯ секции, не новое содержимое: внепоточная коробка, чей
/// содержащий блок лежит ВНЕ копии (у абсолюта нет позиционированного
/// предка внутри неё, у `fixed` — предка-барьера), стоит одна, от своего
/// блока, и повторяться не должна (`abspos-uncontained-text`: PASS один раз).
/// Blink повторяет только фрагменты секции, не её внепоточных потомков —
/// они уходят в `OutOfFlowLayoutPart` один раз от первого фрагмента.
pub(crate) fn drop_uncontained_oof(n: &Node, abs_cb: bool, fixed_cb: bool) -> Option<Node> {
    use crate::style::computed::Position;
    match n {
        Node::Element(k) if k.style.used().position == Some(Position::Absolute) && !abs_cb => None,
        Node::Element(k) if k.style.used().position == Some(Position::Fixed) && !fixed_cb => None,
        Node::Element(k) => {
            let mut c = k.clone();
            let fixed = fixed_cb || fixed_cb_box(&k.style.used());
            let abs = abs_cb || fixed || crate::text::inline::establishes_cb(&k.style.used());
            c.children = k
                .children
                .iter()
                .filter_map(|kid| drop_uncontained_oof(kid, abs, fixed))
                .collect();
            Some(Node::Element(c))
        }
        other => Some(other.clone()),
    }
}

/// Normal-flow paint copy of a paged group: every absolute or fixed box is painted
/// once by the group's native source through its continuation fragments
/// (css-break-3 §parallel flows), so the per-page copy carries none of them.
pub(crate) fn without_oof(n: &Node) -> Option<Node> {
    use crate::style::computed::Position;
    let oof = |n: &Node| {
        matches!(n, Node::Element(k) if matches!(
            k.style.used().position,
            Some(Position::Absolute) | Some(Position::Fixed)
        ))
    };
    if oof(n) {
        return None;
    }
    // Iterative: deep documents must not grow the native stack.
    let mut root = n.clone();
    let mut pending: Vec<&mut Node> = vec![&mut root];
    while let Some(node) = pending.pop() {
        if let Node::Element(e) = node {
            e.children.retain(|c| !oof(c));
            pending.extend(e.children.iter_mut());
        }
    }
    Some(root)
}
