//! Перенос break-before/after с краёв колонок и крайних рядов (§10 CSS Flexbox 1).
use super::{BlockBox, BlockKid, Laid};
impl BlockBox {
    pub(crate) fn edge(&self, last: bool, laid: &Laid) -> (bool, bool) {
        let own = if last {
            (self.own_breaks.1, self.own_breaks.3)
        } else {
            (self.own_breaks.0, self.own_breaks.2)
        };
        let mut propagated = (false, false);
        if let Some(flex) = &self.flex {
            if let Some(frame) = laid.flexes.get(&self.id) {
                // У ряда единица разрыва — линия, а не item. Рекордер уже
                // упорядочил линии вдоль блочной оси фрагментации, включая
                // wrap-reverse. Порядок items определяет края только колонок.
                let edge_line = if last {
                    frame.lines.last()
                } else {
                    frame.lines.first()
                };
                for line in &frame.lines {
                    // Край колонки — её крайний элемент в блочном потоке
                    // фрагментации: тот же порядок, что у разрывов между
                    // соседями линии (Blink переносит front/back линии после
                    // `ApplyReversals`). Анонимный элемент без записанной
                    // геометрии может стоять на краю: переноса нет.
                    let edge_item = line
                        .order
                        .iter()
                        .all(|entry| entry.1.is_some())
                        .then(|| {
                            line.items
                                .iter()
                                .filter_map(|&id| laid.at(id).map(|span| (id, span)))
                                .reduce(|a, b| {
                                    let further = if last { b.1.1 > a.1.1 } else { b.1.0 < a.1.0 };
                                    if further { b } else { a }
                                })
                        })
                        .flatten()
                        .map(|edge| edge.0);
                    for item in &flex.items {
                        let at_edge = if frame.column {
                            edge_item == Some(item.id)
                        } else {
                            edge_line.is_some_and(|edge| edge.index == line.index)
                                && line.items.contains(&item.id)
                        };
                        if at_edge {
                            let value = item.edge(last, laid);
                            propagated.0 |= value.0;
                            propagated.1 |= value.1;
                        }
                    }
                }
            }
        } else if let Some(table) = &self.table {
            propagated = table.edge(last, laid);
        } else if let Some(kid) = if last {
            self.kids.last()
        } else {
            self.kids.first()
        } {
            let edge = if last {
                self.edge_kids.1
            } else {
                self.edge_kids.0
            };
            if edge == Some(kid.id) {
                propagated = kid.edge(last, laid);
            }
        }
        (own.0 || propagated.0, own.1 || propagated.1)
    }
}
impl BlockKid {
    pub(super) fn edge(&self, last: bool, laid: &Laid) -> (bool, bool) {
        if let Some(inner) = &self.inner {
            let mut flexes = Vec::new();
            inner.flexes(&mut flexes);
            if !flexes.is_empty() {
                return inner.edge(last, laid);
            }
        }
        if last {
            (self.force_after, self.avoid_after)
        } else {
            (self.force_before, self.avoid_before)
        }
    }
}

#[cfg(test)]
mod tests;
