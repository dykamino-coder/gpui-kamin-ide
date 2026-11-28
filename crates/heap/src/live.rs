//! Bounded allocation ledger shared by the live profiler and offline folding.
//! Addresses locate current blocks; monotonically increasing IDs identify their
//! lifetime. A successful realloc starts a new lifetime, even at the same address.
//! Missing records make the ledger incomplete rather than silently exact.

use std::collections::VecDeque;
mod table;
use table::Table;

/// Raw return addresses, symbolized outside the allocator hook.
pub const STACK_DEPTH: usize = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stack {
    pub addresses: [usize; STACK_DEPTH],
    pub depth: u8,
}

/// Frame/phase at allocation time. Zero means outside a marked engine phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Context {
    pub frame: u64,
    pub phase: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Block {
    pub id: u64,
    pub address: usize,
    pub bytes: usize,
    pub born: u64,
    pub context: Context,
    pub stack: Stack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Change {
    Allocate(Block),
    Free { id: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Event {
    pub sequence: u64,
    pub change: Change,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Totals {
    /// Requested bytes observed since this session started, not process memory.
    pub live_bytes: u64,
    pub peak_bytes: u64,
    pub peak_sequence: u64,
    pub allocations: u64,
    pub missing_allocations: u64,
    pub history_dropped: u64,
}

/// Preallocated storage. Mutation never grows either container beyond its cap.
pub struct Ledger {
    blocks: Table,
    history: VecDeque<Event>,
    max_blocks: usize,
    max_events: usize,
    next_id: u64,
    sequence: u64,
    totals: Totals,
    context: Context,
}

impl Ledger {
    pub fn new(max_blocks: usize, max_events: usize) -> Self {
        Self {
            blocks: Table::new(max_blocks),
            history: VecDeque::with_capacity(max_events),
            max_blocks,
            max_events,
            next_id: 1,
            sequence: 0,
            totals: Totals::default(),
            context: Context::default(),
        }
    }

    fn event(&mut self, change: Change) {
        self.sequence += 1;
        if self.max_events == 0 {
            self.totals.history_dropped += 1;
            return;
        }
        if self.history.len() == self.max_events {
            self.history.pop_front();
            self.totals.history_dropped += 1;
        }
        self.history.push_back(Event {
            sequence: self.sequence,
            change,
        });
    }

    /// Record a successful allocation. Null and zero-size sentinel addresses are
    /// excluded. A duplicate live address is treated as a lost prior free.
    pub fn allocate(&mut self, address: usize, bytes: usize, stack: Stack) -> Option<u64> {
        if address == 0 || bytes == 0 {
            return None;
        }
        if self.blocks.get(address).is_some() {
            self.totals.missing_allocations += 1;
            self.free(address);
        }
        if self.blocks.len() == self.max_blocks || self.next_id == u64::MAX {
            self.totals.missing_allocations += 1;
            return None;
        }
        let block = Block {
            id: self.next_id,
            address,
            bytes,
            born: self.sequence + 1,
            context: self.context,
            stack,
        };
        self.next_id += 1;
        self.blocks.insert(block);
        self.event(Change::Allocate(block));
        self.totals.allocations += 1;
        self.totals.live_bytes += bytes as u64;
        if self.totals.live_bytes > self.totals.peak_bytes {
            self.totals.peak_bytes = self.totals.live_bytes;
            self.totals.peak_sequence = self.sequence;
        }
        Some(block.id)
    }

    pub fn free(&mut self, address: usize) {
        if let Some(block) = self.blocks.remove(address) {
            self.totals.live_bytes -= block.bytes as u64;
            self.event(Change::Free { id: block.id });
        }
    }

    /// Failed realloc leaves the previous lifetime and its stack untouched.
    pub fn reallocate(&mut self, old: usize, next: usize, bytes: usize, stack: Stack) {
        if next != 0 {
            self.free(old);
            self.allocate(next, bytes, stack);
        }
    }

    pub fn set_context(&mut self, context: Context) {
        self.context = context;
    }
    pub fn context(&self) -> Context {
        self.context
    }

    pub fn totals(&self) -> Totals {
        self.totals
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter()
    }
    pub fn block(&self, address: usize) -> Option<&Block> {
        self.blocks.get(address)
    }
    pub fn history(&self) -> impl Iterator<Item = &Event> {
        self.history.iter()
    }
    pub fn storage_bytes(&self) -> usize {
        self.blocks.storage_bytes() + self.history.capacity() * std::mem::size_of::<Event>()
    }

    /// Reconstruct outside a live hook lock; callers may copy the bounded event
    /// history and use observed_peak directly after releasing their lock.
    pub fn peak(&self) -> Option<Vec<Block>> {
        observed_peak(
            &self.history.iter().copied().collect::<Vec<_>>(),
            self.totals,
        )
    }
}

/// Reconstruct the observed peak only when the complete event prefix survives.
/// Historical allocation data must never acquire owners from a later walk.
pub fn observed_peak(history: &[Event], totals: Totals) -> Option<Vec<Block>> {
    if totals.history_dropped != 0 {
        return None;
    }
    let mut live = std::collections::HashMap::new();
    for event in history
        .iter()
        .take_while(|event| event.sequence <= totals.peak_sequence)
    {
        match event.change {
            Change::Allocate(block) => {
                live.insert(block.id, block);
            }
            Change::Free { id } => {
                live.remove(&id);
            }
        }
    }
    let mut blocks: Vec<_> = live.into_values().collect();
    blocks.sort_by_key(|block| block.id);
    Some(blocks)
}

#[cfg(test)]
mod tests;
