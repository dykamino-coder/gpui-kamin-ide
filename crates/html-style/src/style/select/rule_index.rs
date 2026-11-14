//! Индекс предмета и мемоизация отрицательного фильтра предков в области правил.

use super::{Ancestor, bloom::Bloom};
use crate::budget::admission::{Admission, Buffer, Error};
use crate::style::css::{Rule, Selector};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
mod sharing_facts;
mod storage;
pub use sharing_facts::{SharingFacts, positional_text};

mod bucket;
use bucket::Bucket;

pub struct RuleIndex {
    admission: Admission,
    buckets: [HashMap<String, Bucket>; 4],
    universal: Bucket,
    bulk: RefCell<Option<Box<super::bulk::Table>>>,
    ancestors: Vec<Bloom>,
    /// Решения правил для разделения стиля между элементами.
    pub sharing: Vec<SharingFacts>,
    _lease: Option<crate::budget::LiveReservation>,
}

impl RuleIndex {
    pub fn new(rules: &[Rule], admission: &Admission) -> Result<Self, Error> {
        admission.work(rules.len())?;
        let bytes = rules.iter().try_fold(0usize, |n, r| {
            let key = token(&r.sel).map_or(0, |(_, key)| key.len());
            let ast = storage::bytes(&r.sel, admission)?;
            n.checked_add(key)
                .and_then(|n| n.checked_add(ast))
                .and_then(|n| n.checked_add(384))
                .ok_or(Error::CapacityOverflow)
        })?;
        let mut out = Self {
            admission: admission.clone(),
            buckets: Default::default(),
            universal: Bucket::default(),
            bulk: RefCell::new(None),
            ancestors: Vec::new(),
            sharing: Vec::new(),
            _lease: admission.storage(bytes)?,
        };
        out.ancestors
            .try_reserve_exact(rules.len())
            .map_err(|_| Error::Allocation)?;
        out.sharing
            .try_reserve_exact(rules.len())
            .map_err(|_| Error::Allocation)?;
        for (i, rule) in rules.iter().enumerate() {
            let bucket = if let Some((kind, key)) = token(&rule.sel) {
                out.buckets[kind]
                    .try_reserve(1)
                    .map_err(|_| Error::Allocation)?;
                let key = if kind >= 2 {
                    key.to_ascii_lowercase()
                } else {
                    key.to_string()
                };
                out.buckets[kind].entry(key).or_default()
            } else {
                &mut out.universal
            };
            bucket.rules.try_reserve(1).map_err(|_| Error::Allocation)?;
            bucket.rules.push(i);
            out.ancestors.push(Bloom::requirements(&rule.sel));
            out.sharing.push(SharingFacts::of(rule));
        }
        Ok(out)
    }

    pub fn candidates(&self, me: &Ancestor, path: &[Ancestor]) -> Buffer<usize> {
        me.matching
            .accept(self.try_candidates(me, path))
            .unwrap_or_default()
    }

    /// Вызывается до первого каскада; контекстные селекторы сохраняют обычный путь.
    pub fn prepare_bulk(
        &self,
        root: &markup5ever_rcdom::Handle,
        rules: &[Rule],
        tree: &crate::dom::tree_index::TreeIndex,
        mutation: bool,
    ) -> Result<(), Error> {
        self.reset_bulk();
        *self.bulk.borrow_mut() = if super::bulk::enabled(rules, mutation) {
            let scope = super::has::scope::Scope::new(root, &[], tree)?;
            Some(Box::new(super::bulk::prepare(&scope, rules, tree)?))
        } else {
            None
        };
        Ok(())
    }

    /// Одна область для :has и bulk; состояние не переживает сборку DOM.
    pub fn prepare_document(
        &self,
        root: &markup5ever_rcdom::Handle,
        rules: &[Rule],
        args: &[super::has::HasArg],
        tree: &mut crate::dom::tree_index::TreeIndex,
        mutation: bool,
    ) -> Result<(), Error> {
        self.reset_bulk();
        if !super::bulk::enabled(rules, mutation) {
            return super::has::mark_has(root, args, &mut vec![], tree);
        }
        let mut scope = super::has::scope::Scope::new(root, &[], tree)?;
        if !args.is_empty() {
            super::has::mark_scope(&scope, args, tree)?;
            scope.refresh_marks(tree)?;
        }
        *self.bulk.borrow_mut() = Some(Box::new(super::bulk::prepare(&scope, rules, tree)?));
        Ok(())
    }

    fn reset_bulk(&self) {
        *self.bulk.borrow_mut() = None;
        self.universal.clear();
        for bucket in self.buckets.iter().flat_map(|buckets| buckets.values()) {
            bucket.clear();
        }
    }

    fn try_candidates(&self, me: &Ancestor, path: &[Ancestor]) -> Result<Buffer<usize>, Error> {
        let bloom = path
            .last()
            .and_then(|a| a.ancestor_bloom)
            .unwrap_or_else(|| path.iter().fold(Bloom::default(), |b, a| b.with_node(a)));
        let mut out = Buffer::default();
        let bulk = self.bulk.borrow();
        let bulk = bulk.as_deref();
        if !self.universal.rules.is_empty() {
            self.universal
                .append(bloom, &self.ancestors, &mut out, &self.admission, bulk)?;
        }
        let mut append = |kind: usize, key: &str| -> Result<(), Error> {
            if let Some(bucket) = self.buckets[kind].get(key) {
                bucket.append(bloom, &self.ancestors, &mut out, &self.admission, bulk)?;
            }
            Ok(())
        };
        if let Some(id) = &me.id {
            append(0, id)?;
        }
        for class in &me.classes {
            append(1, class)?;
        }
        append(2, &super::ascii_lower(&me.tag))?;
        for (name, _) in &me.attrs {
            append(3, &super::ascii_lower(name))?;
        }
        if let Some(bulk) = bulk {
            for &i in bulk.candidates(&me.key) {
                out.push(i, &self.admission)?;
            }
        }
        self.admission.work(out.len())?;
        out.sort_unstable();
        let mut unique = Buffer::default();
        for &i in out.iter() {
            if unique.last() != Some(&i) {
                unique.push(i, &self.admission)?;
            }
        }
        Ok(unique)
    }
}

fn token(sel: &Selector) -> Option<(usize, &str)> {
    if let Some(id) = &sel.id {
        return Some((0, id));
    }
    if let Some(class) = sel.classes.first() {
        return Some((1, class));
    }
    if let Some(tag) = &sel.tag {
        return Some((2, tag));
    }
    sel.attrs.first().map(|a| (3, a.name.as_str()))
}

#[cfg(test)]
mod completeness;
#[cfg(test)]
mod tests;
