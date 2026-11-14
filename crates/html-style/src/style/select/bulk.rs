//! Готовые кандидаты сложных правил для cold-прохода; живут только до конца каскада.

use super::{bits::Bits, has::scope::Scope, sets};
use crate::budget::{
    LiveReservation,
    admission::{Buffer, Error},
};
use crate::dom::tree_index::{NodeKey, TreeIndex};
use crate::style::css::Rule;
use std::collections::HashMap;

pub(super) struct Table {
    handled: Bits,
    nodes: HashMap<NodeKey, Buffer<usize>>,
    leases: Buffer<Option<LiveReservation>>,
}

impl Table {
    pub fn handles(&self, rule: usize) -> bool {
        self.handled.contains(rule)
    }

    pub fn candidates(&self, key: &NodeKey) -> &[usize] {
        self.nodes.get(key).map_or(&[], |rules| &rules[..])
    }

    fn insert(&mut self, key: NodeKey, rule: usize, tree: &TreeIndex) -> Result<(), Error> {
        let admission = tree.admission();
        admission.work(1)?;
        if !self.nodes.contains_key(&key) {
            let lease = admission.storage(128)?;
            self.nodes.try_reserve(1).map_err(|_| Error::Allocation)?;
            self.leases.push(lease, &admission)?;
        }
        self.nodes.entry(key).or_default().push(rule, &admission)
    }
}

fn eligible(rule: &Rule) -> bool {
    sets::edge(&rule.sel).is_some() && sets::eligible(&rule.sel)
}

/// Малые таблицы и изменения остаются на element-major пути с Invalidation.
/// Bulk окупается при большом количестве повторяемых проверок цепочек.
pub(super) fn enabled(rules: &[Rule], mutation: bool) -> bool {
    !mutation && rules.iter().filter(|r| eligible(r)).take(64).count() >= 64
}

pub(super) fn prepare(scope: &Scope, rules: &[Rule], tree: &TreeIndex) -> Result<Table, Error> {
    let admission = tree.admission();
    let compounds = sets::Cache::new(
        rules.iter().filter(|r| eligible(r)).map(|r| &r.sel),
        &admission,
    )?;
    let mut table = Table {
        handled: Bits::new(rules.len(), &admission)?,
        nodes: HashMap::new(),
        leases: Buffer::default(),
    };
    for (i, rule) in rules.iter().enumerate() {
        if !eligible(rule) {
            continue;
        }
        admission.work(1)?;
        let found = sets::matches(scope, &rule.sel, tree, &compounds)?;
        table.handled.insert(i);
        for node in found.ones() {
            table.insert(NodeKey::of(&scope.records[node].handle), i, tree)?;
        }
    }
    Ok(table)
}
