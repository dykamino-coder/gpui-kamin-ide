//! Готовые кандидаты сложных правил для cold-прохода; живут только до конца каскада.

use super::{bits::Bits, has::scope::Scope, sets};
use crate::budget::{
    LiveReservation,
    admission::{Buffer, Error},
};
use crate::dom::tree_index::{NodeKey, TreeIndex};
use crate::style::css::Rule;
use std::{collections::HashMap, ops::Range};

pub(super) struct Table {
    handled: Bits,
    nodes: HashMap<NodeKey, Range<usize>>,
    rules: Buffer<usize>,
    _nodes: Option<LiveReservation>,
}

impl Table {
    pub fn handles(&self, rule: usize) -> bool {
        self.handled.contains(rule)
    }

    pub fn candidates(&self, key: &NodeKey) -> &[usize] {
        self.nodes
            .get(key)
            .map_or(&[], |range| &self.rules[range.clone()])
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
    let mut handled = Bits::new(rules.len(), &admission)?;
    let mut matches: Buffer<(Range<usize>, Bits)> = Buffer::default();
    let mut counts = Buffer::default();
    counts.reserve(scope.records.len(), &admission)?;
    for _ in &scope.records[..] {
        counts.push(0usize, &admission)?;
    }
    for (i, rule) in rules.iter().enumerate() {
        if !eligible(rule) {
            continue;
        }
        admission.work(1)?;
        let found = sets::matches(scope, &rule.sel, tree, &compounds)?;
        handled.insert(i);
        if found.is_empty() {
            continue;
        }
        // Соседние одинаковые множества считают размеры строк один раз.
        if let Some((range, previous)) = matches.last_mut()
            && range.end == i
            && previous.same(&found, &admission)?
        {
            range.end = i + 1;
        } else {
            matches.push((i..i + 1, found), &admission)?;
        }
    }
    for (range, found) in &matches[..] {
        for node in found.ones() {
            admission.work(1)?;
            counts[node] = counts[node]
                .checked_add(range.len())
                .ok_or(Error::CapacityOverflow)?;
        }
    }
    let distinct = counts.iter().filter(|&&count| count > 0).count();
    let lease = admission.storage(distinct.checked_mul(128).ok_or(Error::CapacityOverflow)?)?;
    let mut nodes = HashMap::new();
    nodes.try_reserve(distinct).map_err(|_| Error::Allocation)?;
    let mut total = 0usize;
    admission.work(counts.len())?;
    for (i, count) in counts.iter_mut().enumerate() {
        let end = total.checked_add(*count).ok_or(Error::CapacityOverflow)?;
        if *count > 0 {
            nodes.insert(NodeKey::of(&scope.records[i].handle), total..end);
        }
        *count = total;
        total = end;
    }
    // Один backing store не создаёт отдельные Vec и ledger-владельцев на каждый узел.
    let mut out = Table {
        handled,
        nodes,
        rules: Buffer::default(),
        _nodes: lease,
    };
    out.rules.reserve(total, &admission)?;
    admission.work(total)?;
    for _ in 0..total {
        out.rules.push(0, &admission)?;
    }
    for (range, found) in &matches[..] {
        for node in found.ones() {
            admission.work(range.len())?;
            for rule in range.clone() {
                out.rules[counts[node]] = rule;
                counts[node] += 1;
            }
        }
    }
    Ok(out)
}
