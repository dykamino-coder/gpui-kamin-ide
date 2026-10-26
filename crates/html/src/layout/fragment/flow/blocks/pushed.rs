//! Адрес роста по началу normal box или его независимого overflow-потока.

use super::{BlockBox, Laid, Pushed};
use crate::tolerance::COORD_EPS;

impl BlockBox {
    /// Самая внешняя коробка стопки, чей разложенный border box начинается
    /// ровно в `a` (`laid` — разложенная копия в координатах потока): перед ней
    /// встаёт распорка роста. Коробка, накрывающая `a`, — спуск в её стопку;
    /// монолит и коробка без своей стопки спуска не дают. В сетке — ряд,
    /// начинающийся в `a` (`GridBox::pushed_at`).
    pub(crate) fn pushed_at(&self, a: f32, laid: &Laid) -> Option<Pushed> {
        if let Some(t) = &self.table {
            return t.pushed_at(a, laid);
        }
        if let Some(g) = &self.grid {
            return g.pushed_at(self.id, a, laid);
        }
        if let Some(f) = &self.flex {
            return f.pushed_at(self.id, a, laid);
        }
        for k in &self.kids {
            let Some((top, bottom)) = laid.at(k.id) else {
                continue;
            };
            // Элемент потока flex-колонки сдвигается вместе со своим полем:
            // граница перед ним — начало его margin box (`walk_kids`).
            let start = if self.flex_gap.is_some() {
                laid.margin_box(k.id).map_or(top, |m| m.0)
            } else {
                top
            };
            if (top - a).abs() < COORD_EPS || (start - a).abs() < COORD_EPS {
                return Some(Pushed::Box(k.id));
            }
            let end = k
                .inner
                .as_ref()
                .and_then(|b| crate::layout::fragment::flow::flow_end(b, laid))
                .unwrap_or(bottom);
            if top < a
                && a < end - COORD_EPS
                && let Some(found) = k.inner.as_ref().and_then(|b| b.pushed_at(a, laid))
            {
                return Some(found);
            }
        }
        laid.lines
            .get(&self.id)?
            .iter()
            .find(|p| (p.at - a).abs() < COORD_EPS)
            .cloned()
            .map(Pushed::Line)
    }
}
