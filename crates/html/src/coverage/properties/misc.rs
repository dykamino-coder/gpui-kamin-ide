//! CSS property coverage for misc: kept in registry order.

use super::{m, part};
use crate::coverage::Prop;

pub(super) const PROPERTIES: &[Prop] = &[
    // --- Прочее ----------------------------------------------------------
    m("cursor", "pointer"),
    m("object-fit", "cover"),
    part(
        "pointer-events",
        "none",
        "снимает наведение, курсор и выделение; сквозного клика к элементу под ним нет",
    ),
    m("table-layout", "fixed"),
];
