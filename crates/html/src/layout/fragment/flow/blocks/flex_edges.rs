//! Перенос break-before/after с действительных крайних flex-линий (§10 CSS Flexbox 1).
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
                for line in &frame.lines {
                    let members: Vec<_> = flex
                        .items
                        .iter()
                        .filter(|i| line.items.contains(&i.id))
                        .collect();
                    let edge: Vec<_> = if frame.column {
                        let at = members
                            .iter()
                            .filter_map(|i| {
                                laid.at(i.id).map(|p| (i.id, if last { p.1 } else { p.0 }))
                            })
                            .reduce(|a, b| {
                                if (last && b.1 > a.1) || (!last && b.1 < a.1) {
                                    b
                                } else {
                                    a
                                }
                            });
                        members
                            .into_iter()
                            .filter(|i| at.is_some_and(|a| a.0 == i.id))
                            .collect()
                    } else if (last && frame.lines.last().is_some_and(|l| l.index == line.index))
                        || (!last && frame.lines.first().is_some_and(|l| l.index == line.index))
                    {
                        members
                    } else {
                        Vec::new()
                    };
                    for item in edge {
                        let value = item.edge(last, laid);
                        propagated.0 |= value.0;
                        propagated.1 |= value.1;
                    }
                }
            }
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
