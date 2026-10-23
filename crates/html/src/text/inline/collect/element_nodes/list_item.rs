//! Дети строчного элемента при сборе кусков: у строчного `display: list-item` маркер
//! (css-lists-3: `outside` вычисляется в `inside`) встаёт первым узлом и идёт обычным
//! текстовым потоком пункта.
use crate::dom::Element;
use crate::style::computed::Computed;
use crate::text::inline::Piece;
use crate::text::inline::collect::CollectCx;

pub(super) fn children(
    e: &Element,
    merged: &Computed,
    atom: &mut dyn FnMut(&Element, &crate::text::inline::atom_parent::AtomCx) -> Option<Piece>,
    has_text: bool,
    cx: &mut CollectCx,
) -> Vec<Piece> {
    let doc = cx.layout.doc().clone();
    let vcx = doc.frame_values();
    // CSS Lists 3: outside computes to inside for an inline list item.
    if e.style.cascaded().inline_display == Some(true)
        && e.style.cascaded().display == Some(crate::style::computed::Display::ListItem)
        && let Some(marker) =
            crate::layout::list::list_item::marker::inline_marker(e, merged, merged, vcx)
    {
        let mut nodes = Vec::with_capacity(e.children.len() + 1);
        nodes.push(marker);
        nodes.extend(e.children.iter().cloned());
        super::super::collect_with_empty_metrics(&nodes, merged, atom, has_text, cx)
    } else {
        super::super::collect_with_empty_metrics(&e.children, merged, atom, has_text, cx)
    }
}
