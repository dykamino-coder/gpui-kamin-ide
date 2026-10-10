//! Элемент покраски правил промежутков.
// owner: A

mod lifecycle;

use crate::paint::gap_rules::{GapItems, GapRuleSpec};
use gpui::IntoElement;

/// Слой линеек промежутков. Забирает буфер проб в `paint` (к этому моменту
/// prepaint всех детей уже прошёл — так же работает `EdgePainter`), строит
/// геометрию промежутков по границам элементов и красит отрезки линеек
/// (css-gaps-1 §geometry, §break, §inset, §visibility-items, §lists).
pub struct GapRulePainter {
    pub(crate) items: GapItems,
    pub(crate) spec: GapRuleSpec,
}

impl GapRulePainter {
    pub fn new(items: GapItems, spec: GapRuleSpec) -> Self {
        GapRulePainter { items, spec }
    }
}

impl IntoElement for GapRulePainter {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
