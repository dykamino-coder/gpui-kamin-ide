//! Главная копия даёт измеренную высоту и разрывы стопки; её final geometry определяет
//! распорки `push::Regrow`. Копии пересобираются до устойчивого плана роста этой стопки
//! (css-break-3 §box-splitting; Blink `FinishFragmentation`).

use crate::layout::fragment::types::BreaksAt;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, AvailableSpace, Window, px, size};

mod copies;
mod extent;
mod growth;
mod items;
mod nest;
mod numeric;
pub(crate) use nest::{first_row_overflow, fragmentainer_extent};
mod owners;
mod progress;
mod record;
mod trial;

impl ColumnStack {
    /// Раскладка главной копии ребёнка `i` в колонку до укладки.
    ///
    /// Копия может сама содержать многоколоночник, и его раскладка идёт внутри
    /// этой: кадр, живущий на время вложенной раскладки, держится малым, а
    /// разбор её итога — отдельным кадром (`record_main`) после возврата.
    pub(crate) fn lay_out_main(&mut self, i: usize, window: &mut Window, cx: &mut App) {
        let axis = self.axis;
        let c = &mut self.children[i];
        let (measured, w) = match (c.measure, c.breaks_w) {
            (Some(w), _) => (true, w),
            (None, Some(BreaksAt::Width(w))) => (false, w),
            _ => return,
        };
        crate::work::add!(MulticolChildLayouts, 1);
        let (sz, content) = c.el.layout_as_root_with_content(
            if axis.is_vertical() {
                size(AvailableSpace::MaxContent, AvailableSpace::Definite(px(w)))
            } else {
                size(AvailableSpace::Definite(px(w)), AvailableSpace::MaxContent)
            },
            window,
            cx,
        );
        self.record_main(i, measured, (sz, content), window);
    }

    /// Применить заявки и переложить только изменённые деревья.
    fn grow_once(&mut self, window: &mut Window, cx: &mut App) -> bool {
        let local_grows = self.growth_requests();
        let mut dirty = vec![false; self.children.len()];
        for (index, grows) in local_grows.into_iter().enumerate() {
            if let Some(r) = self.children[index].regrow.as_mut() {
                dirty[index] = r.push_many(grows);
            }
        }
        if !dirty.contains(&true) {
            return false;
        }
        for (i, _) in dirty.iter().enumerate().filter(|d| *d.1) {
            copies::rebuild(&mut self.children[i]);
            self.lay_out_main(i, window, cx);
        }
        self.reserve_row_copies();
        self.reserve_overflow_copies();
        true
    }
}
