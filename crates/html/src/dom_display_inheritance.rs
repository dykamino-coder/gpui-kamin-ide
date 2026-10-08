//! Resolve display inheritance before box fixup, including implicit HTML display values.

use super::{Element, Node};
use crate::computed::Display;

/// `display` родителя вместе с метками ролей: то, что переносит `inherit`.
type DisplayOf = (
    Option<Display>,
    Option<bool>,
    Option<u8>,
    Option<u8>,
    Option<bool>,
);

/// `display: inherit` — вычисленное значение ДОМ-родителя (CSS 2.1 §6.2.1).
///
/// Прежде бит `inh::DISPLAY` решался при сборке (`inline::inherit`) от
/// РЕНДЕР-родителя, а табличная починка (`fixup_row_children`,
/// `wrap_anon_tables`) смотрит `e.style.display` раньше и видела `None`:
/// `#test {display: inherit}` в `.tr {display: table-row}` становился блоком
/// с красным фоном и рамкой внутри анонимной ячейки, а не рядом без ячеек
/// (`empty-cells-applies-to-017`). Explicit values retain that early
/// resolution; implicit values are recovered from HTML box metadata.
pub(super) fn resolve_display_inherit(nodes: &mut [Node], parent: DisplayOf) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        if el.style.inherit_bits & crate::computed::inh::DISPLAY != 0 {
            el.style.display = parent.0;
            el.style.inline_display = parent.1;
            el.style.row_group_kind = parent.2;
            el.style.col_role = parent.3;
            el.style.is_caption = parent.4;
            el.style.inherit_bits &= !crate::computed::inh::DISPLAY;
        }
        let own = display_of(el);
        resolve_display_inherit(&mut el.children, own);
    }
}

fn display_of(el: &Element) -> DisplayOf {
    let style = &el.style;
    let own = (
        style.display,
        style.inline_display,
        style.row_group_kind,
        style.col_role,
        style.is_caption,
    );
    if style.display.is_some() {
        return own;
    }
    // CSS 2.1 section 6.2.1 inherits the computed value, not the absence
    // of a declaration. Our implicit HTML display lives in the tag/inline
    // metadata, so a default div must transmit block even to an inline
    // pseudo-element, and a default span must transmit inline to a div.
    if el.inline {
        return (Some(Display::InlineBlock), Some(true), None, None, None);
    }
    match el.tag.as_str() {
        "table" => (Some(Display::Table), None, None, None, None),
        "tr" => (Some(Display::TableRow), None, None, None, None),
        "td" | "th" => (Some(Display::TableCell), None, None, None, None),
        "thead" | "tbody" | "tfoot" => {
            let kind = match el.tag.as_str() {
                "thead" => 0,
                "tfoot" => 2,
                _ => 1,
            };
            (Some(Display::TableRowGroup), None, Some(kind), None, None)
        }
        "caption" => (Some(Display::Block), None, None, None, Some(true)),
        "col" => (Some(Display::None), None, None, Some(0), None),
        "colgroup" => (Some(Display::None), None, None, Some(1), None),
        "li" => (Some(Display::ListItem), None, None, None, None),
        _ => (Some(Display::Block), None, None, None, None),
    }
}
