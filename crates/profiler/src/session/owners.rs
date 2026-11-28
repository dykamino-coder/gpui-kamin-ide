//! Match owner-reported ranges to stable allocation identities across the walk.

use kamin_heap::{Ops, live::Block};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Owner {
    pub allocation: u64,
    pub path: String,
    pub observed_bytes: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Ownership {
    pub begin_epoch: u64,
    pub owners: Vec<Owner>,
    pub reported_bytes: BTreeMap<String, u64>,
    pub unreported_observed_bytes: u64,
    pub unmatched_claims: u64,
    pub claims_dropped: u64,
}

pub(super) struct Claims {
    items: Vec<(&'static str, usize, usize)>,
    reported_bytes: BTreeMap<String, u64>,
    dropped: u64,
    unmatched: u64,
}

pub(super) fn walk(before: &[Block], walk: impl FnOnce(&mut Ops<'_>)) -> Claims {
    let mut items = Vec::with_capacity(before.len());
    let mut seen = HashSet::with_capacity(before.len());
    let mut unmatched = 0;
    let mut sink = |path, address: *const u8, bytes| {
        if let Some(block) = containing(before, address as usize, bytes) {
            // Only observed allocations consume storage. Repeated shared or
            // interior claims keep the first owner, as Ops does for Rc/Arc.
            if seen.insert(block.id) {
                items.push((path, address as usize, bytes));
            }
        } else {
            unmatched += 1;
        }
    };
    let mut ops = Ops::with_sink(&mut sink);
    walk(&mut ops);
    let reported_bytes = ops.report().entries().clone();
    Claims {
        items,
        reported_bytes,
        dropped: 0,
        unmatched,
    }
}

fn containing(blocks: &[Block], address: usize, bytes: usize) -> Option<&Block> {
    let index = blocks
        .partition_point(|block| block.address <= address)
        .checked_sub(1)?;
    let block = &blocks[index];
    let offset = address.checked_sub(block.address)?;
    (offset < block.bytes && bytes <= block.bytes - offset).then_some(block)
}

pub(super) fn match_claims(
    epoch: u64,
    before: &[Block],
    after: &[Block],
    claims: Claims,
) -> Ownership {
    let mut result = Ownership {
        begin_epoch: epoch,
        owners: Vec::new(),
        reported_bytes: claims.reported_bytes,
        unreported_observed_bytes: after.iter().map(|block| block.bytes as u64).sum(),
        unmatched_claims: claims.unmatched,
        claims_dropped: claims.dropped,
    };
    let mut seen = HashSet::new();
    for (path, address, bytes) in claims.items {
        let old = containing(before, address, bytes);
        let current = containing(after, address, bytes);
        if let (Some(old), Some(current)) = (old, current) {
            if old.id == current.id {
                if seen.insert(current.id) {
                    result.owners.push(Owner {
                        allocation: current.id,
                        path: path.to_owned(),
                        observed_bytes: current.bytes as u64,
                    });
                    result.unreported_observed_bytes -= current.bytes as u64;
                }
                continue;
            }
        }
        result.unmatched_claims += 1;
    }
    result.owners.sort_by_key(|owner| owner.allocation);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use kamin_heap::live::Stack;

    #[test]
    fn interior_shared_claims_match_once_and_address_reuse_is_not_an_owner() {
        let block = |id, address| Block {
            id,
            address,
            bytes: 32,
            born: 1,
            context: Default::default(),
            stack: Stack::default(),
        };
        let before = [block(1, 16), block(2, 64)];
        let after = [block(1, 16), block(3, 64), block(4, 128)];
        let claims = Claims {
            items: vec![("a", 24, 8), ("b", 16, 32), ("stale", 64, 32)],
            reported_bytes: BTreeMap::new(),
            dropped: 0,
            unmatched: 0,
        };
        let result = match_claims(1, &before, &after, claims);
        assert_eq!(
            result.owners,
            [Owner {
                allocation: 1,
                path: "a".into(),
                observed_bytes: 32
            }]
        );
        assert_eq!(result.unreported_observed_bytes, 64);
        assert_eq!(result.unmatched_claims, 1);
    }
    #[test]
    fn unsampled_claims_do_not_hide_the_observed_owner() {
        let before = [Block {
            id: 1,
            address: 1024,
            bytes: 32,
            born: 1,
            context: Default::default(),
            stack: Stack::default(),
        }];
        let claims = walk(&before, |ops| {
            for address in 1..1000 {
                ops.block(address as *const u8, 1);
            }
            ops.at("observed", |ops| {
                ops.block(1024 as *const u8, 32);
            });
            ops.block(1032 as *const u8, 8);
        });
        let result = match_claims(1, &before, &before, claims);
        assert_eq!(result.owners.len(), 1);
        assert_eq!(result.owners[0].path, "observed");
        assert_eq!(result.unmatched_claims, 999);
        assert_eq!(result.claims_dropped, 0);
        assert_eq!(result.unreported_observed_bytes, 0);
    }
}
