//! Карта компаундов набора правил: заимствованный AST, коллизии проверяются целиком.
//! Биты повторяемых компаундов живут одну фазу над неизменным деревом и метками.

use super::*;
use crate::budget::{LiveReservation, admission::Admission};
use std::{
    cell::OnceCell,
    collections::HashMap,
    hash::{Hash, Hasher},
};

struct Entry<'a> {
    selector: &'a Selector,
    uses: usize,
    bits: OnceCell<Bits>,
}

#[derive(Default)]
pub(in crate::style::select) struct Cache<'a> {
    entries: HashMap<u64, Buffer<Entry<'a>>>,
    leases: Buffer<Option<LiveReservation>>,
}

fn fingerprint(sel: &Selector) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    sel.tag.hash(&mut hash);
    sel.id.hash(&mut hash);
    sel.classes.hash(&mut hash);
    for attr in &sel.attrs {
        attr.name.hash(&mut hash);
        attr.op.hash(&mut hash);
        attr.ci.hash(&mut hash);
    }
    sel.pseudo.hash(&mut hash);
    sel.also.hash(&mut hash);
    sel.universal.hash(&mut hash);
    hash.finish()
}

fn same(a: &Selector, b: &Selector) -> bool {
    a.tag == b.tag
        && a.tag_namespace == b.tag_namespace
        && a.argument_namespace == b.argument_namespace
        && a.arguments == b.arguments
        && a.id == b.id
        && a.classes == b.classes
        && a.attrs == b.attrs
        && a.pseudo == b.pseudo
        && a.also == b.also
        && a.universal == b.universal
}

impl<'a> Cache<'a> {
    /// Один обход AST набора таблиц; не клонирует селекторы и цепочки.
    pub fn new(
        selectors: impl Iterator<Item = &'a Selector>,
        admission: &Admission,
    ) -> Result<Self, Error> {
        let mut out = Self::default();
        for sel in selectors {
            let mut current = Some(sel);
            while let Some(sel) = current {
                admission.work(1)?;
                let key = fingerprint(sel);
                if !out.entries.contains_key(&key) {
                    let lease = admission.storage(128)?;
                    out.entries.try_reserve(1).map_err(|_| Error::Allocation)?;
                    out.leases.push(lease, admission)?;
                }
                let bucket = out.entries.entry(key).or_default();
                if let Some(entry) = bucket.iter_mut().find(|e| same(e.selector, sel)) {
                    entry.uses += 1;
                } else {
                    bucket.push(
                        Entry {
                            selector: sel,
                            uses: 1,
                            bits: OnceCell::new(),
                        },
                        admission,
                    )?;
                }
                current = edge(sel).map(|(left, _)| left);
            }
        }
        Ok(out)
    }

    pub fn compound(&self, scope: &Scope, sel: &Selector, tree: &TreeIndex) -> Result<Bits, Error> {
        tree.admission().work(1)?;
        let entry = self
            .entries
            .get(&fingerprint(sel))
            .and_then(|bucket| bucket.iter().find(|e| same(e.selector, sel)))
            .filter(|e| e.uses > 1);
        let Some(entry) = entry else {
            return compound(scope, sel, tree);
        };
        if entry.bits.get().is_none() {
            let bits = compound(scope, sel, tree)?;
            let _ = entry.bits.set(bits);
        }
        entry
            .bits
            .get()
            .expect("initialized compound")
            .copy(&tree.admission())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_identity_ignores_edges_but_keeps_namespace_and_prepared_arguments() {
        let a = Selector::parse("a > i:nth-child(2)").unwrap();
        let mut b = Selector::parse("b + i:nth-child(2)").unwrap();
        assert!(same(&a, &b));
        b.tag_namespace = crate::style::css::TypeNamespace::Foreign;
        assert!(!same(&a, &b));
        b.tag_namespace = a.tag_namespace;
        b.arguments.clear();
        assert!(!same(&a, &b));
    }
}
