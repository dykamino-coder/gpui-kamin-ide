//! A sampled peak is the largest simultaneous weighted live set, which can
//! differ from the largest observed byte set. Lost prefixes stay unavailable.

use crate::{
    fold::Amount,
    live::{Block, Change, Event, Totals},
};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Peak {
    pub sequence: u64,
    pub amount: Amount,
    pub blocks: Vec<Block>,
}

pub fn estimated(history: &[Event], totals: Totals, mean: usize) -> Option<Peak> {
    if totals.history_dropped != 0 {
        return None;
    }
    let mut live = HashMap::new();
    let mut current = Amount::default();
    let (mut best, mut sequence) = (Amount::default(), 0);
    for event in history {
        match event.change {
            Change::Allocate(block) => {
                let amount = Amount::block(block.bytes, mean);
                live.insert(block.id, amount);
                current.add(amount);
            }
            Change::Free { id } => {
                if let Some(amount) = live.remove(&id) {
                    current.observed_blocks -= amount.observed_blocks;
                    current.observed_bytes -= amount.observed_bytes;
                    current.estimated_blocks -= amount.estimated_blocks;
                    current.estimated_bytes -= amount.estimated_bytes;
                }
            }
        }
        if current.estimated_bytes > best.estimated_bytes {
            best = current;
            sequence = event.sequence;
        }
    }
    // Replay once, after locating the peak, rather than copying a growing live
    // table for every new high-water mark.
    let mut blocks = HashMap::new();
    for event in history
        .iter()
        .take_while(|event| event.sequence <= sequence)
    {
        match event.change {
            Change::Allocate(block) => {
                blocks.insert(block.id, block);
            }
            Change::Free { id } => {
                blocks.remove(&id);
            }
        }
    }
    let mut blocks: Vec<_> = blocks.into_values().collect();
    blocks.sort_by_key(|block| block.id);
    Some(Peak {
        sequence,
        amount: best,
        blocks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{Ledger, Stack};

    #[test]
    fn sampled_peak_can_differ_from_observed_peak_and_late_owners_are_absent() {
        let mut ledger = Ledger::new(4, 16);
        ledger.allocate(16, 512, Stack::default());
        ledger.free(16);
        for address in [16, 32, 48] {
            ledger.allocate(address, 1, Stack::default());
        }
        let history: Vec<_> = ledger.history().copied().collect();
        let peak = estimated(&history, ledger.totals(), 4096).unwrap();
        assert_eq!(peak.amount.observed_bytes, 3);
        assert_eq!(peak.blocks.len(), 3);
        assert!(peak.amount.estimated_bytes > 12288.0);
        let exact = estimated(&history, ledger.totals(), 0).unwrap();
        assert_eq!(exact.amount.observed_bytes, 512);
        assert_eq!(exact.blocks, ledger.peak().unwrap());
        let lost = Totals {
            history_dropped: 1,
            ..ledger.totals()
        };
        assert!(estimated(&history, lost, 4096).is_none());
    }
}
