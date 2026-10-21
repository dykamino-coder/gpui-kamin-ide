//! Внепоточные потомки повторяемых секций таблицы (css-tables-3 §repeated-headers): копия
//! шапки/подвала несёт только оформление, и абсолют или `fixed`, чей содержащий блок лежит
//! вне копии, из неё удаляется; здесь же признак коробки — содержащего блока `fixed`.

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
        Node::Element(k) if k.style.position == Some(Position::Absolute) && !abs_cb => None,
        Node::Element(k) if k.style.position == Some(Position::Fixed) && !fixed_cb => None,
        Node::Element(k) => {
            let mut c = k.clone();
            let fixed = fixed_cb || fixed_cb_box(&k.style);
            let abs = abs_cb || fixed || crate::text::inline::establishes_cb(&k.style);
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
