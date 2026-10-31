//! Индекс предмета и мемоизация отрицательного фильтра предков в области правил.

use super::{Ancestor, bloom::Bloom};
use crate::budget::admission::{Admission, Buffer, Error};
use crate::style::css::{Rule, Selector};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
mod storage;

struct Candidates {
    rules: Vec<usize>,
    _lease: Option<crate::budget::LiveReservation>,
}

#[derive(Default)]
struct Bucket {
    rules: Vec<usize>,
    cache: RefCell<HashMap<Bloom, Rc<Candidates>>>,
}

impl Bucket {
    fn append(
        &self,
        bloom: Bloom,
        requirements: &[Bloom],
        out: &mut Buffer<usize>,
        admission: &Admission,
    ) -> Result<(), Error> {
        let cached = self.cache.borrow().get(&bloom).cloned();
        let candidates = if let Some(cached) = cached {
            cached
        } else {
            admission.work(
                self.rules
                    .len()
                    .checked_mul(2)
                    .ok_or(Error::CapacityOverflow)?,
            )?;
            let count = self
                .rules
                .iter()
                .filter(|&&i| bloom.contains(requirements[i]))
                .count();
            let bytes = count
                .checked_mul(std::mem::size_of::<usize>())
                .and_then(|n| n.checked_add(192))
                .ok_or(Error::CapacityOverflow)?;
            let lease = admission.storage(bytes)?;
            let mut rules = Vec::new();
            rules
                .try_reserve_exact(count)
                .map_err(|_| Error::Allocation)?;
            for &i in &self.rules {
                if bloom.contains(requirements[i]) {
                    rules.push(i);
                }
            }
            let candidates = Rc::new(Candidates {
                rules,
                _lease: lease,
            });
            self.cache
                .borrow_mut()
                .try_reserve(1)
                .map_err(|_| Error::Allocation)?;
            self.cache.borrow_mut().insert(bloom, candidates.clone());
            candidates
        };
        admission.work(candidates.rules.len())?;
        for &i in &candidates.rules {
            out.push(i, admission)?;
        }
        Ok(())
    }
}

pub struct RuleIndex {
    admission: Admission,
    buckets: [HashMap<String, Bucket>; 4],
    universal: Bucket,
    ancestors: Vec<Bloom>,
    pub link_state: Vec<bool>,
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
            ancestors: Vec::new(),
            link_state: Vec::new(),
            _lease: admission.storage(bytes)?,
        };
        out.ancestors
            .try_reserve_exact(rules.len())
            .map_err(|_| Error::Allocation)?;
        out.link_state
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
            out.link_state.push(mentions_link_state(&rule.sel));
        }
        Ok(out)
    }

    pub fn candidates(&self, me: &Ancestor, path: &[Ancestor]) -> Buffer<usize> {
        me.matching
            .accept(self.try_candidates(me, path))
            .unwrap_or_default()
    }

    fn try_candidates(&self, me: &Ancestor, path: &[Ancestor]) -> Result<Buffer<usize>, Error> {
        let bloom = path
            .last()
            .and_then(|a| a.ancestor_bloom)
            .unwrap_or_else(|| path.iter().fold(Bloom::default(), |b, a| b.with_node(a)));
        let mut out = Buffer::default();
        if !self.universal.rules.is_empty() {
            self.universal
                .append(bloom, &self.ancestors, &mut out, &self.admission)?;
        }
        let mut append = |kind: usize, key: &str| -> Result<(), Error> {
            if let Some(bucket) = self.buckets[kind].get(key) {
                bucket.append(bloom, &self.ancestors, &mut out, &self.admission)?;
            }
            Ok(())
        };
        if let Some(id) = &me.id {
            append(0, id)?;
        }
        for class in &me.classes {
            append(1, class)?;
        }
        append(2, &me.tag.to_ascii_lowercase())?;
        for (name, _) in &me.attrs {
            append(3, &name.to_ascii_lowercase())?;
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

fn mentions_link_state(sel: &Selector) -> bool {
    sel.pseudo.iter().chain(&sel.also).any(|p| {
        let p = p.to_ascii_lowercase();
        p.contains("link") || p.contains("visited")
    }) || sel
        .prev
        .as_deref()
        .is_some_and(|(s, _)| mentions_link_state(s))
        || sel
            .ancestor
            .as_deref()
            .is_some_and(|(s, _)| mentions_link_state(s))
}

#[cfg(test)]
mod completeness;
#[cfg(test)]
mod tests;
