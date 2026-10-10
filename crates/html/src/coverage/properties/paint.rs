//! CSS property coverage for paint: kept in registry order.

use super::{m, part};
use crate::coverage::Prop;

pub(super) const PROPERTIES: &[Prop] = &[
    // --- Фон, рамки, тени ------------------------------------------------
    part(
        "background",
        "#123456",
        "из сокращения читаются цвет, картинка, повтор и размер; позиция и слои — нет",
    ),
    m("background-color", "#123456"),
    m("background-image", "url(logo.png)"),
    m("border", "1px solid #333"),
    m("border-top", "1px solid #333"),
    m("border-right", "1px solid #333"),
    m("border-bottom", "1px solid #333"),
    m("border-left", "1px solid #333"),
    m("border-width", "2px"),
    m("border-top-width", "2px"),
    m("border-right-width", "2px"),
    m("border-bottom-width", "2px"),
    m("border-left-width", "2px"),
    m("border-inline", "1px solid #333"),
    m("border-block", "1px solid #333"),
    m("border-color", "#333"),
    m("border-top-color", "#333"),
    m("border-right-color", "#333"),
    m("border-bottom-color", "#333"),
    m("border-left-color", "#333"),
    part(
        "border-style",
        "dotted",
        "`double`, `groove` и `ridge` рисуются сплошной: рельефа в конвейере нет",
    ),
    m("border-radius", "6px"),
    m("border-top-left-radius", "6px"),
    m("border-top-right-radius", "6px"),
    m("border-bottom-right-radius", "6px"),
    m("border-bottom-left-radius", "6px"),
    m("border-collapse", "collapse"),
    m("border-spacing", "4px"),
    m("outline", "solid #333"),
    m("outline-width", "2px"),
    m("outline-color", "#333"),
    m("outline-offset", "2px"),
    m("box-shadow", "inset 0 2px 4px #0004"),
    m("backdrop-filter", "blur(8px)"),
];
