//! Сопоставление цепочки селектора над множествами элементов одной области.

use super::{bits::Bits, has::scope::Scope, relations::project};
use crate::budget::admission::{Buffer, Error};
use crate::dom::tree_index::TreeIndex;
use crate::style::css::{PseudoKind, Selector};

pub(super) fn edge(sel: &Selector) -> Option<(&Selector, char)> {
    if let Some((left, direct)) = sel.prev.as_deref() {
        Some((left, if *direct { '+' } else { '~' }))
    } else {
        sel.ancestor
            .as_deref()
            .map(|(left, direct)| (left, if *direct { '>' } else { ' ' }))
    }
}

/// Эти компаунды имеют одинаковую семантику в предметной и предковой позиции.
/// Остальные исполняет обычный matcher с полным контекстом.
pub(super) fn eligible(sel: &Selector) -> bool {
    if sel.prev.is_some() && sel.ancestor.is_some() {
        return false;
    }
    let pseudos = sel.pseudo.iter().chain(&sel.also);
    let local = pseudos.clone().all(|p| {
        matches!(
            sel.pseudo_of(p).1,
            PseudoKind::Has(_) | PseudoKind::Structural(..) | PseudoKind::Empty | PseudoKind::Root
        )
    });
    let ancestor_also = sel
        .ancestor
        .as_deref()
        .is_none_or(|(s, _)| s.also.is_empty());
    local && ancestor_also && edge(sel).is_none_or(|(left, _)| eligible(left))
}

fn compound(scope: &Scope, sel: &Selector, tree: &TreeIndex) -> Result<Bits, Error> {
    let admission = tree.admission();
    let mut bits = Bits::new(scope.records.len(), &admission)?;
    let candidates = scope
        .index
        .candidates(sel, scope.records.len(), &admission)?;
    if candidates.is_empty() {
        return Ok(bits);
    }
    let mut subject = sel.clone();
    subject.prev = None;
    subject.ancestor = None;
    for &i in candidates.iter() {
        if scope.match_at(&subject, i, tree)? {
            bits.insert(i);
        }
    }
    Ok(bits)
}

pub(super) fn matches(scope: &Scope, sel: &Selector, tree: &TreeIndex) -> Result<Bits, Error> {
    let admission = tree.admission();
    let mut chain = Buffer::default();
    let mut left = sel;
    while let Some((next, relation)) = edge(left) {
        chain.push((left, relation), &admission)?;
        left = next;
    }
    let mut found = compound(scope, left, tree)?;
    for &(right, relation) in chain.iter().rev() {
        if found.is_empty() {
            break;
        }
        found = project(scope, &found, relation, false, &admission)?;
        found.intersect(&compound(scope, right, tree)?, &admission)?;
    }
    Ok(found)
}

pub(super) fn anchors(scope: &Scope, sel: &Selector, tree: &TreeIndex) -> Result<Bits, Error> {
    let admission = tree.admission();
    let mut found = compound(scope, sel, tree)?;
    let mut right = sel;
    while let Some((left, relation)) = edge(right) {
        if found.is_empty() {
            break;
        }
        found = project(scope, &found, relation, true, &admission)?;
        if left
            .attrs
            .iter()
            .any(|a| a.name == super::has::HAS_SENTINEL)
        {
            break;
        }
        found.intersect(&compound(scope, left, tree)?, &admission)?;
        right = left;
    }
    Ok(found)
}

#[cfg(test)]
mod tests;
