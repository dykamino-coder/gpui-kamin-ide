//! Snapshot aggregation: observed requested bytes and Poisson estimates stay
//! separate from process commit, owner-reported bytes and logical GPU resources.

use crate::live::{Block, Stack};
use std::collections::BTreeMap;

pub mod classify;
pub mod peak;
pub use classify::Site;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Amount {
    pub observed_blocks: u64,
    pub observed_bytes: u64,
    pub estimated_blocks: f64,
    pub estimated_bytes: f64,
}

impl Amount {
    pub fn block(bytes: usize, mean: usize) -> Self {
        if bytes == 0 {
            return Self::default();
        }
        let probability = if mean == 0 {
            1.0
        } else {
            -(-(bytes as f64) / mean as f64).exp_m1()
        };
        Self {
            observed_blocks: 1,
            observed_bytes: bytes as u64,
            estimated_blocks: 1.0 / probability,
            estimated_bytes: bytes as f64 / probability,
        }
    }

    pub fn add(&mut self, other: Self) {
        self.observed_blocks += other.observed_blocks;
        self.observed_bytes += other.observed_bytes;
        self.estimated_blocks += other.estimated_blocks;
        self.estimated_bytes += other.estimated_bytes;
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub total: Amount,
    pub by_site: BTreeMap<String, Amount>,
    pub by_subsystem: BTreeMap<String, Amount>,
    pub by_owned_type: BTreeMap<String, Amount>,
    pub unresolved_blocks: u64,
    pub truncated_stacks: u64,
}

/// Symbol lookup is supplied by the platform service, outside allocator hooks.
/// One lookup per distinct stack, including inline frames in leaf-to-root order.
pub fn fold(
    blocks: &[Block],
    mean: usize,
    mut symbolize: impl FnMut(&Stack) -> Vec<String>,
) -> Report {
    let mut sites = BTreeMap::new();
    let mut report = Report::default();
    for block in blocks {
        if block.bytes == 0 {
            continue;
        }
        let key = &block.stack.addresses
            [..usize::from(block.stack.depth).min(block.stack.addresses.len())];
        let site = sites
            .entry(key.to_vec())
            .or_insert_with(|| classify::classify(symbolize(&block.stack)));
        let amount = Amount::block(block.bytes, mean);
        report.total.add(amount);
        report
            .by_site
            .entry(site.name.clone())
            .or_default()
            .add(amount);
        report
            .by_subsystem
            .entry(site.subsystem.clone())
            .or_default()
            .add(amount);
        if let Some(owned) = &site.owned_type {
            report
                .by_owned_type
                .entry(owned.clone())
                .or_default()
                .add(amount);
        }
        if site.name.starts_with('?') {
            report.unresolved_blocks += 1;
        }
        if usize::from(block.stack.depth) >= block.stack.addresses.len() {
            report.truncated_stacks += 1;
        }
    }
    report
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Delta {
    pub observed_blocks: i128,
    pub observed_bytes: i128,
    pub estimated_blocks: f64,
    pub estimated_bytes: f64,
}

pub fn diff(
    before: &BTreeMap<String, Amount>,
    after: &BTreeMap<String, Amount>,
) -> BTreeMap<String, Delta> {
    before
        .keys()
        .chain(after.keys())
        .map(|key| {
            let a = before.get(key).copied().unwrap_or_default();
            let b = after.get(key).copied().unwrap_or_default();
            (
                key.clone(),
                Delta {
                    observed_blocks: i128::from(b.observed_blocks) - i128::from(a.observed_blocks),
                    observed_bytes: i128::from(b.observed_bytes) - i128::from(a.observed_bytes),
                    estimated_blocks: b.estimated_blocks - a.estimated_blocks,
                    estimated_bytes: b.estimated_bytes - a.estimated_bytes,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisson_estimates_exact_folding_and_signed_diff() {
        let block = |id, bytes| Block {
            id,
            address: id as usize * 16,
            bytes,
            born: id,
            context: Default::default(),
            stack: Stack::default(),
        };
        let blocks = [block(1, 64), block(2, 128)];
        let mut lookups = 0;
        let report = fold(&blocks, 0, |_| {
            lookups += 1;
            vec!["kamin_html::layout::build".into()]
        });
        assert_eq!(lookups, 1);
        assert_eq!(report.total.observed_bytes, 192);
        assert_eq!(report.total.estimated_bytes, 192.0);
        assert_eq!(report.by_subsystem["layout"], report.total);
        let estimated = Amount::block(64, 4096);
        assert!((estimated.estimated_bytes - 4128.083333).abs() < 0.0001);
        let end = fold(&blocks[..1], 0, |_| {
            vec!["kamin_html::layout::build".into()]
        });
        assert_eq!(
            diff(&report.by_site, &end.by_site)["kamin_html::layout::build"].observed_bytes,
            -128
        );
        assert_eq!(
            diff(&end.by_site, &BTreeMap::new())["kamin_html::layout::build"].observed_bytes,
            -64
        );
    }
}
