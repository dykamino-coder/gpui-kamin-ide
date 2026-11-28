//! Bounded raw snapshot interchange. The same allocation identities and events
//! feed live DevTools and offline reports; derived peaks are always recomputed.

use crate::{report::Build, session::Snapshot, symbols::Module};
use kamin_heap::live::{Block, Change, STACK_DEPTH};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
};

pub const SCHEMA: u32 = 1;
pub const MAX_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
pub struct Archive {
    pub schema: u32,
    pub build: Build,
    pub modules: Vec<Module>,
    pub snapshot: Snapshot,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl Archive {
    pub fn new(build: Build, modules: Vec<Module>, snapshot: Snapshot) -> Self {
        Self {
            schema: SCHEMA,
            build,
            modules,
            snapshot,
        }
    }

    pub fn write(&self, output: &mut impl Write) -> io::Result<()> {
        let _suppress = crate::allocation::Suppress::enter();
        self.validate()?;
        serde_json::to_writer(output, self).map_err(io::Error::other)
    }

    pub fn read(input: &mut impl Read) -> io::Result<Self> {
        let _suppress = crate::allocation::Suppress::enter();
        let mut data = Vec::new();
        input.take(MAX_BYTES + 1).read_to_end(&mut data)?;
        if data.len() as u64 > MAX_BYTES {
            return Err(invalid("snapshot exceeds byte limit"));
        }
        let mut archive: Self = serde_json::from_slice(&data).map_err(invalid_json)?;
        archive.validate()?;
        let snapshot = &mut archive.snapshot;
        snapshot.peak = kamin_heap::live::observed_peak(&snapshot.history, snapshot.totals);
        snapshot.estimated_peak = kamin_heap::fold::peak::estimated(
            &snapshot.history,
            snapshot.totals,
            snapshot.sampling_mean,
        );
        Ok(archive)
    }

    fn validate(&self) -> io::Result<()> {
        let s = &self.snapshot;
        if self.schema != SCHEMA
            || self.modules.len() > 256
            || s.live.len() > 262_144
            || s.history.len() > 524_288
            || self.build.features.len() > 256
        {
            return Err(invalid("snapshot schema or capacity out of bounds"));
        }
        let mut ids = BTreeSet::new();
        let mut end = 0;
        let mut bytes = 0u64;
        for block in &s.live {
            validate_block(block, s.sequence)?;
            if block.address < end || !ids.insert(block.id) {
                return Err(invalid("overlapping allocations or duplicate IDs"));
            }
            end = block.address + block.bytes;
            bytes = bytes
                .checked_add(block.bytes as u64)
                .ok_or_else(|| invalid("byte overflow"))?;
        }
        if bytes != s.totals.live_bytes
            || s.totals.peak_bytes < bytes
            || s.totals.peak_sequence > s.sequence
            || s.totals.history_dropped.checked_add(s.history.len() as u64) != Some(s.sequence)
        {
            return Err(invalid("inconsistent snapshot totals"));
        }
        let mut sequence = s.totals.history_dropped;
        let mut last_id = 0;
        let mut live = BTreeMap::new();
        for event in &s.history {
            sequence += 1;
            if event.sequence != sequence {
                return Err(invalid("non-contiguous event sequence"));
            }
            match event.change {
                Change::Allocate(block) => {
                    validate_block(&block, sequence)?;
                    if block.born != sequence || block.id <= last_id {
                        return Err(invalid("non-monotonic allocation identity"));
                    }
                    last_id = block.id;
                    live.insert(block.id, block);
                }
                Change::Free { id } => {
                    if live.remove(&id).is_none() && s.totals.history_dropped == 0 {
                        return Err(invalid("free of an unknown allocation"));
                    }
                }
            }
        }
        if s.totals.history_dropped == 0
            && (live.len() != s.live.len()
                || s.live
                    .iter()
                    .any(|block| live.get(&block.id) != Some(block)))
        {
            return Err(invalid("event replay differs from live snapshot"));
        }
        if let Some(owners) = &s.owners {
            let mut seen = BTreeSet::new();
            if owners.begin_epoch > s.epoch
                || owners.owners.len() > s.live.len()
                || owners.reported_bytes.len() > 65_536
            {
                return Err(invalid("ownership capacity or epoch out of bounds"));
            }
            for owner in &owners.owners {
                if !ids.contains(&owner.allocation)
                    || !seen.insert(owner.allocation)
                    || owner.path.len() > 4096
                {
                    return Err(invalid("ownership does not match snapshot identities"));
                }
            }
        }
        Ok(())
    }
}

fn invalid_json(error: serde_json::Error) -> io::Error {
    invalid(&error.to_string())
}

fn validate_block(block: &Block, sequence: u64) -> io::Result<()> {
    if block.id == 0
        || block.address == 0
        || block.bytes == 0
        || block.born == 0
        || block.born > sequence
        || block.address.checked_add(block.bytes).is_none()
        || usize::from(block.stack.depth) > STACK_DEPTH
    {
        return Err(invalid("invalid allocation record"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
