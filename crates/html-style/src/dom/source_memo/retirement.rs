//! Retire replaced output edges after every new ownership claim is known.
use super::{CascadeEntry, NodeKey, RootOutput, SourceMemo};
use crate::budget::admission::Error;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub(super) fn removed(
    base: &SourceMemo,
    replacements: &HashMap<NodeKey, Rc<CascadeEntry>>,
    output: &RootOutput,
) -> Result<HashSet<NodeKey>, Error> {
    let mut claims = HashSet::new();
    claim(&mut claims, &output.ownership.children)?;
    let mut pending = Vec::new();
    enqueue(&mut pending, &base.output.ownership.children)?;
    for entry in replacements.values() {
        claim(&mut claims, &entry.ownership.children)?;
        claims.try_reserve(1).map_err(|_| Error::Allocation)?;
        claims.insert(entry.node);
        if let Some(old) = base.get(entry.node) {
            enqueue(&mut pending, &old.ownership.children)?;
        }
    }
    let mut retired = HashSet::new();
    while let Some(node) = pending.pop() {
        if claims.contains(&node) || retired.contains(&node) {
            continue;
        }
        retired.try_reserve(1).map_err(|_| Error::Allocation)?;
        retired.insert(node);
        if let Some(entry) = base.get(node) {
            enqueue(&mut pending, &entry.ownership.children)?;
        }
    }
    Ok(retired)
}

fn claim(claims: &mut HashSet<NodeKey>, nodes: &[NodeKey]) -> Result<(), Error> {
    claims
        .try_reserve(nodes.len())
        .map_err(|_| Error::Allocation)?;
    claims.extend(nodes.iter().copied());
    Ok(())
}

fn enqueue(pending: &mut Vec<NodeKey>, nodes: &[NodeKey]) -> Result<(), Error> {
    pending
        .try_reserve(nodes.len())
        .map_err(|_| Error::Allocation)?;
    pending.extend_from_slice(nodes);
    Ok(())
}
