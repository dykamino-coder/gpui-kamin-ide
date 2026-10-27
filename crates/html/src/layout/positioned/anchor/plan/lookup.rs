//! Поиск приемлемого якоря текущего кадра в порядке дерева.

use super::super::settle::DefaultAnchor;
use super::super::{AnchorPlan, AnchorRec};

impl AnchorPlan {
    pub(in crate::layout::positioned::anchor) fn rec(
        &self,
        name: Option<&str>,
    ) -> Option<AnchorRec> {
        let f = &self.layout.anchors;
        let named = |name: &str| {
            f.named_seq
                .borrow()
                .iter()
                .filter(|(n, seq, rec)| {
                    n == name
                        && (!f.out_of_flow.borrow().contains_key(&rec.id) || *seq < self.seq)
                        && self.acceptable(rec)
                })
                .max_by_key(|(_, seq, _)| *seq)
                .map(|(_, _, rec)| *rec)
        };
        match name {
            Some(n) => named(n),
            None => match &self.default_anchor {
                Some(DefaultAnchor::Named(n)) => named(n),
                Some(DefaultAnchor::Implicit(id)) => f
                    .implicit
                    .borrow()
                    .get(id)
                    .copied()
                    .filter(|r| self.acceptable(r)),
                None => None,
            },
        }
    }

    pub(in crate::layout::positioned::anchor) fn rec_current(
        &self,
        name: Option<&str>,
    ) -> Option<AnchorRec> {
        self.rec(name)
    }

    /// §target, acceptable anchor element: «same original containing block»
    /// либо элемент, порождающий содержащий блок якоря, сам приемлем — то
    /// есть содержащий блок коробки лежит в цепочке содержащих блоков
    /// якоря (или сам является якорем: неявный якорь позиционированного
    /// хозяина). Начальный содержащий блок завершает цепочку;
    /// звено без пробы (строчный или атомарный содержащий блок) — цепочка
    /// неизвестна, якорь не отвергается (`no-anchor-anchor-center`).
    fn acceptable(&self, r: &AnchorRec) -> bool {
        if r.id == self.key {
            return false;
        }
        if r.id == self.cb_node {
            return true;
        }
        let mut n = r.cb;
        let f = &self.layout.anchors;
        let mut visited = std::collections::HashSet::new();
        while visited.insert(n) {
            if n == self.key && n != 0
                || f.out_of_flow
                    .borrow()
                    .get(&n)
                    .is_some_and(|seq| *seq >= self.seq)
            {
                return false;
            }
            if n == self.cb_node || n == 0 && self.fixed {
                return true;
            }
            if n == 0 {
                return false;
            }
            match f.cb_parent.borrow().get(&n).copied() {
                Some(p) => n = p,
                None => return true,
            }
        }
        false
    }
}
