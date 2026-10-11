//! CSS property coverage for box model: kept in registry order.

use super::m;
use crate::coverage::Prop;

pub(super) const PROPERTIES: &[Prop] = &[
    // --- Бокс-модель -----------------------------------------------------
    m("box-sizing", "border-box"),
    m("width", "100px"),
    m("height", "100px"),
    m("min-width", "10px"),
    m("min-height", "10px"),
    m("max-width", "500px"),
    m("max-height", "500px"),
    m("inline-size", "100px"),
    m("block-size", "100px"),
    m("min-inline-size", "10px"),
    m("min-block-size", "10px"),
    m("max-inline-size", "500px"),
    m("max-block-size", "500px"),
    m("aspect-ratio", "16 / 9"),
    m("padding", "4px"),
    m("padding-top", "4px"),
    m("padding-right", "4px"),
    m("padding-bottom", "4px"),
    m("padding-left", "4px"),
    m("padding-inline", "4px"),
    m("padding-block", "4px"),
    m("padding-inline-start", "4px"),
    m("padding-inline-end", "4px"),
    m("padding-block-start", "4px"),
    m("padding-block-end", "4px"),
    m("margin", "4px"),
    m("margin-top", "4px"),
    m("margin-right", "4px"),
    m("margin-bottom", "4px"),
    m("margin-left", "4px"),
    m("margin-inline", "4px"),
    m("margin-block", "4px"),
    m("margin-inline-start", "4px"),
    m("margin-inline-end", "4px"),
    m("margin-block-start", "4px"),
    m("margin-block-end", "4px"),
];
