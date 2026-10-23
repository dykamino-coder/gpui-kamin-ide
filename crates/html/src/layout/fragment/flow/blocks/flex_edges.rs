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
                    let edge_item = if last {
                        line.order.last()
                    } else {
                        line.order.first()
                    };
                    for item in &flex.items {
                        let at_edge = if frame.column {
                            edge_item.is_some_and(|edge| edge.1 == Some(item.id))
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
