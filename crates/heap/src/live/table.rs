//! Fixed open-addressed storage: churn never reallocates inside allocator hooks.

use super::Block;

#[derive(Clone, Copy, Default)]
enum Slot {
    #[default]
    Empty,
    Live(Block),
}

pub(super) struct Table {
    slots: Box<[Slot]>,
    len: usize,
}

impl Table {
    pub(super) fn new(max_blocks: usize) -> Self {
        let slots = max_blocks
            .checked_mul(2)
            .and_then(usize::checked_next_power_of_two)
            .expect("validated profiler block capacity")
            .max(1);
        Self {
            slots: vec![Slot::Empty; slots].into_boxed_slice(),
            len: 0,
        }
    }

    fn indices(&self, address: usize) -> impl Iterator<Item = usize> + use<> {
        let mask = self.slots.len() - 1;
        let hash = hash(address);
        (0..self.slots.len()).map(move |offset| hash.wrapping_add(offset) & mask)
    }

    pub(super) fn get(&self, address: usize) -> Option<&Block> {
        for index in self.indices(address) {
            match &self.slots[index] {
                Slot::Empty => return None,
                Slot::Live(block) if block.address == address => return Some(block),
                _ => {}
            }
        }
        None
    }

    pub(super) fn insert(&mut self, block: Block) {
        for index in self.indices(block.address) {
            if !matches!(self.slots[index], Slot::Live(_)) {
                self.slots[index] = Slot::Live(block);
                self.len += 1;
                return;
            }
        }
        unreachable!("ledger caps live entries at half the fixed table size")
    }

    pub(super) fn remove(&mut self, address: usize) -> Option<Block> {
        for index in self.indices(address) {
            match self.slots[index] {
                Slot::Empty => return None,
                Slot::Live(block) if block.address == address => {
                    self.close_hole(index);
                    self.len -= 1;
                    return Some(block);
                }
                _ => {}
            }
        }
        None
    }

    fn close_hole(&mut self, mut hole: usize) {
        let mask = self.slots.len() - 1;
        let mut next = (hole + 1) & mask;
        while let Slot::Live(block) = self.slots[next] {
            let home = hash(block.address) & mask;
            if (next.wrapping_sub(home) & mask) >= (hole.wrapping_sub(home) & mask) {
                self.slots[hole] = self.slots[next];
                hole = next;
            }
            next = (next + 1) & mask;
        }
        self.slots[hole] = Slot::Empty;
    }

    pub(super) fn len(&self) -> usize {
        self.len
    }
    pub(super) fn storage_bytes(&self) -> usize {
        self.slots.len() * std::mem::size_of::<Slot>()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Block> {
        self.slots.iter().filter_map(|slot| match slot {
            Slot::Live(block) => Some(block),
            _ => None,
        })
    }
}

fn hash(address: usize) -> usize {
    let mut value = address as u64;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    (value ^ (value >> 31)) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::Stack;

    #[test]
    fn collisions_tombstones_and_churn_keep_fixed_storage() {
        let mut table = Table::new(2);
        let storage = table.slots.as_ptr();
        let block = |address| Block {
            id: address as u64,
            address,
            bytes: 8,
            born: 1,
            context: Default::default(),
            stack: Stack::default(),
        };
        for index in 1..100 {
            let address = index * 32;
            table.insert(block(address));
            table.insert(block(address + 32));
            assert!(table.remove(address).is_some());
            assert_eq!(table.get(address + 32).unwrap().address, address + 32);
            table.remove(address + 32);
            assert_eq!(table.slots.as_ptr(), storage);
            assert_eq!(table.len(), 0);
        }
    }
}
