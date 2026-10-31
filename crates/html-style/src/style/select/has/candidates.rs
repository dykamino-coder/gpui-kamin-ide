//! Кандидаты предмета :has() и линейная мемоизация простых отношений.

use super::*;
use crate::budget::admission::Buffer;
use scope::Scope;

pub(super) fn candidates(
    scope: &Scope,
    sel: &Selector,
    tree: &TreeIndex,
) -> Result<Buffer<usize>, Error> {
    let admission = tree.admission();
    scope.index.candidates(sel, scope.records.len(), &admission)
}

fn sentinel(sel: &Selector) -> bool {
    sel.attrs.iter().any(|a| a.name == HAS_SENTINEL)
}

/// Единственный компаунд после якоря не зависит от выбранного якоря:
/// положительный и отрицательный результат проверяются один раз на кандидата.
pub(super) fn simple(sel: &Selector) -> Option<(char, Selector)> {
    let relation = if let Some((s, direct)) = sel.ancestor.as_deref() {
        if !sentinel(s) || sel.prev.is_some() {
            return None;
        }
        if *direct { '>' } else { ' ' }
    } else if let Some((s, direct)) = sel.prev.as_deref() {
        if !sentinel(s) {
            return None;
        }
        if *direct { '+' } else { '~' }
    } else {
        return None;
    };
    let mut subject = sel.clone();
    subject.ancestor = None;
    subject.prev = None;
    Some((relation, subject))
}

pub(super) fn mark_simple(
    scope: &Scope,
    relation: char,
    subject: &Selector,
    candidates: &[usize],
    found: &mut super::marks::Marks,
    tree: &TreeIndex,
) -> Result<(), Error> {
    let admission = tree.admission();
    for &candidate in candidates {
        if !scope.match_at(subject, candidate, tree)? {
            continue;
        }
        let mut anchor = if matches!(relation, '+' | '~') {
            scope.previous(candidate)
        } else {
            scope.records[candidate].parent
        };
        while let Some(i) = anchor {
            admission.work(1)?;
            if !found.propagate(i, &admission)? {
                break;
            }
            anchor = match relation {
                ' ' => scope.records[i].parent,
                '~' => scope.previous(i),
                _ => None,
            };
        }
    }
    Ok(())
}

pub(super) fn mark_complex(
    scope: &Scope,
    sibling: bool,
    sel: &Selector,
    candidates: &[usize],
    found: &mut super::marks::Marks,
    tree: &TreeIndex,
) -> Result<(), Error> {
    let admission = tree.admission();
    if candidates.is_empty() {
        return Ok(());
    }
    for (anchor, record) in scope.records.iter().enumerate() {
        if found.contains(anchor) {
            continue;
        }
        admission.work(1)?;
        let (start, end) = if sibling {
            (
                record.end,
                record
                    .parent
                    .map_or(scope.records.len(), |p| scope.records[p].end),
            )
        } else {
            (anchor + 1, record.end)
        };
        let begin = candidates.partition_point(|&i| i < start);
        let _overlay = tree.matching.anchor(record.all[record.pos].key.clone());
        for &candidate in &candidates[begin..] {
            if candidate >= end {
                break;
            }
            if scope.match_at(sel, candidate, tree)? {
                found.propagate(anchor, &admission)?;
                break;
            }
        }
    }
    Ok(())
}
