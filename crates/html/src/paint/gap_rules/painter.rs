//! GPUI-элемент `GapRulePainter` — слой линеек промежутков контейнера: в `paint`, когда
//! prepaint всех детей уже прошёл, забирает буфер проб, строит геометрию промежутков и красит
//! отрезки линеек (css-gaps-1 §geometry, §break, §inset, §visibility-items, §lists).
//! Параметры — буфер проб и `GapRuleSpec`; жизненный цикл — в `lifecycle`.

mod lifecycle;
mod painting;

use crate::paint::gap_rules::{GapItems, GapRuleSpec};

/// Слой линеек промежутков. Забирает буфер проб в `paint` (к этому моменту
/// prepaint всех детей уже прошёл — так же работает `EdgePainter`), строит
/// геометрию промежутков по границам элементов и красит отрезки линеек
/// (css-gaps-1 §geometry, §break, §inset, §visibility-items, §lists).
pub struct GapRulePainter {
    pub(crate) paint: std::rc::Rc<crate::layout::cx::deferred::Deferred>,
    pub(crate) items: GapItems,
    pub(crate) spec: GapRuleSpec,
}

impl GapRulePainter {
    pub fn new(
        items: GapItems,
        spec: GapRuleSpec,
        paint: std::rc::Rc<crate::layout::cx::deferred::Deferred>,
    ) -> Self {
        GapRulePainter { items, spec, paint }
    }
}
