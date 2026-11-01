//! Пробы по измеренным строкам: DOM и GPUI-элементы живут до выбранной высоты.

use super::{ColumnStack, copies, trial::restore_geometry};
use crate::layout::fragment::{flow::blocks::Laid, plan, token::Error, types::Kid};
use crate::layout::multicol::column_stack::balance::search::{Trial, min_shortage, search};
use gpui::{App, Window};
mod lines;
use lines::Lines;

struct Input {
    geometry: Kid,
    laid: Option<Laid>,
    lines: Option<Lines>,
}

impl ColumnStack {
    /// Неподдерживаемая зависимость отвергает всю числовую транзакцию.
    pub(super) fn numeric_balance(
        &mut self,
        start: f32,
        ceiling: f32,
        capped: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let mut input: Vec<_> = self
            .children
            .iter()
            .map(|c| Input {
                geometry: c.kid(),
                laid: c.regrow.as_ref().map(|r| r.laid.clone()),
                lines: Lines::measure(c, self.axis, window),
            })
            .collect();
        if input.iter().all(|i| i.lines.is_none()) {
            return false;
        }
        let previous = self.forced_h;
        let forced = Self::forced_breaks(&self.kid_geoms()).saturating_add(1);
        let mut unsupported = false;
        let chosen = search(start, ceiling, capped, |height| {
            self.reset_numeric(&mut input);
            self.forced_h = Some(height);
            let settled = self.settle_numeric(&mut input);
            unsupported |= settled.is_none();
            let kids = self.kid_geoms();
            let (cols, shortage, plan) =
                Self::fill_avoiding(&kids, &|_| height, self.column_space(usize::MAX));
            let avoid = (forced < self.count)
                .then(|| Self::first_avoid_violation(&kids, &plan))
                .flatten()
                .and_then(|bad| Self::avoid_shortage(&kids, &plan, bad, height));
            let oof = self.oof_balance_trial(&plan, height, self.count.max(forced));
            Trial {
                fits: !unsupported
                    && settled == Some(Ok(()))
                    && oof.as_ref().is_ok_and(|r| r.0)
                    && avoid.is_none()
                    && plan.completion == plan::Completion::Complete
                    && cols <= self.count.max(forced),
                shortage: min_shortage([avoid, oof.ok().and_then(|r| r.1), Some(shortage)]),
                value: settled,
            }
        });
        self.reset_numeric(&mut input);
        self.forced_h = previous;
        if unsupported {
            return false;
        }
        let height = match chosen {
            Ok((height, Some(Ok(())))) => height,
            Ok((_, Some(Err(error)))) | Err(error) => {
                self.balance_failure = Some(error);
                return true;
            }
            _ => return false,
        };
        self.forced_h = Some(height);
        if self.settle_numeric(&mut input) != Some(Ok(())) {
            self.reset_numeric(&mut input);
            self.forced_h = previous;
            return false;
        }
        // Только выбранные распорки становятся входом дерева отрисовки.
        for (index, entry) in input.iter_mut().enumerate() {
            restore_geometry(&mut self.children[index], &entry.geometry);
            if let (Some(r), Some(laid)) = (&mut self.children[index].regrow, &entry.laid) {
                r.laid.clone_from(laid);
            }
            if entry.lines.as_ref().is_some_and(Lines::commit) {
                copies::rebuild(&mut self.children[index]);
                self.lay_out_main(index, window, cx);
            }
        }
        self.reserve_row_copies();
        true
    }

    fn reset_numeric(&mut self, input: &mut [Input]) {
        for (child, entry) in self.children.iter_mut().zip(input) {
            restore_geometry(child, &entry.geometry);
            if let (Some(r), Some(laid)) = (&mut child.regrow, &entry.laid) {
                r.laid.clone_from(laid);
            }
            if let Some(lines) = &mut entry.lines {
                lines.reset();
            }
        }
    }

    fn settle_numeric(&mut self, input: &mut [Input]) -> Option<Result<(), Error>> {
        let mut seen = std::collections::HashSet::new();
        loop {
            if self
                .children
                .iter()
                .any(|c| !c.h.is_finite() || !c.over.is_finite())
            {
                return Some(Err(Error::InvalidGeometry));
            }
            // Исходная геометрия неизменна; вектор распорок полностью задаёт
            // состояние сертифицированных IFC, без сериализации всего Laid.
            let state: Vec<_> = input
                .iter()
                .filter_map(|i| i.lines.as_ref())
                .flat_map(|l| l.extra.iter().map(|v| v.to_bits()))
                .collect();
            let grows = self.growth_requests();
            if grows.iter().all(Vec::is_empty) {
                return Some(Ok(()));
            }
            // Проверить все адреса до применения: проба не правит общие LinePush.
            for (entry, grows) in input.iter().zip(&grows) {
                if !grows.is_empty() && !entry.lines.as_ref().is_some_and(|l| l.accepts(grows)) {
                    return None;
                }
            }
            for ((entry, child), grows) in input.iter_mut().zip(&mut self.children).zip(grows) {
                if !grows.is_empty() {
                    entry.lines.as_mut()?.grow(child, &entry.geometry, grows);
                }
            }
            if !seen.insert(state) {
                return Some(Err(Error::NoProgress));
            }
        }
    }
}

#[cfg(test)]
mod tests;
