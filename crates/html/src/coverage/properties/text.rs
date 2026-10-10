//! CSS property coverage for text: kept in registry order.

use super::{imp, m};
use crate::coverage::Prop;

pub(super) const PROPERTIES: &[Prop] = &[
    // --- Текст -----------------------------------------------------------
    m("color", "#123456"),
    m("font", "italic 700 14px/1.4 monospace"),
    m("font-size", "14px"),
    m("font-weight", "700"),
    m("font-style", "italic"),
    m("font-family", "Segoe UI, sans-serif"),
    m("line-height", "1.4"),
    m("letter-spacing", "0.5px"),
    m("word-spacing", "2px"),
    m("text-align", "justify"),
    m("text-decoration", "underline"),
    m("text-decoration-line", "line-through"),
    m("text-transform", "uppercase"),
    m("text-indent", "12px"),
    m("text-overflow", "ellipsis"),
    m("white-space", "pre-line"),
    m("word-break", "break-all"),
    m("line-break", "anywhere"),
    imp(
        "overflow-wrap",
        "переносчик GPUI рвёт слово, которое иначе не влезает, ВСЕГДА — отличить `normal` от `break-word` нечем, и значение ни на что не влияет",
    ),
    imp(
        "word-wrap",
        "старое имя `overflow-wrap` — то же ограничение",
    ),
    m("text-wrap", "nowrap"),
    m("vertical-align", "middle"),
    m("-webkit-line-clamp", "2"),
    m("list-style", "square"),
    m("list-style-type", "lower-roman"),
];
