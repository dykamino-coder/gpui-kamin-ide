//! Heap retention without garbage collection: conservative scanning of live
//! blocks (as in Boehm GC and DMD) produces block-to-block edges. Thread stacks
//! and module statics provide roots. Analysis computes shortest root paths and
//! a dominator tree with retained sizes, as in Gecko ComputeShortestPaths and
//! DominatorTree and the Chrome Memory panel.
//!
//! The caller supplies live blocks and memory access through `Memory` on demand;
//! these algorithms have no platform dependencies or background work.
//!
//! Node IDs start with address-sorted blocks, followed by a virtual root, one
//! node per root region, and an unrooted node. The latter reaches blocks held
//! by unscanned stacks, OS memory, or leaked `Rc` cycles.

mod analyze;
mod read;
mod scan;
mod unrooted;
mod write;

#[cfg(test)]
mod tests;

/// A live heap block: address and requested size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub address: usize,
    pub bytes: usize,
}

/// A non-heap root region whose words can hold blocks: a stack, writable
/// module section, or thread-local storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub name: String,
    pub start: usize,
    pub bytes: usize,
}

/// Read blocks and root regions as pointer-sized words.
pub trait Memory {
    /// Aligned words in `[address, address + bytes)`; ignore a partial trailing word.
    fn words(&self, address: usize, bytes: usize) -> &[usize];
}

/// No edge or parent.
pub const NONE: u32 = u32::MAX;

/// A block entered through the unrooted node as a source with no incoming edge.
pub const FLAG_UNHELD: u8 = 1;
/// A representative of a cycle with no external holder, such as a leaked `Rc` cycle.
pub const FLAG_CYCLE: u8 = 2;
pub use read::ReadLimits;

/// The retention graph and its analysis.
#[derive(Debug, Default)]
pub struct Graph {
    pub blocks: Vec<Block>,
    pub regions: Vec<Region>,
    /// Node `i` edges occupy `edge_start[i]..edge_start[i + 1]`.
    pub edge_start: Vec<u32>,
    pub edge_to: Vec<u32>,
    /// Byte offset of the pointer word inside the edge source.
    pub edge_offset: Vec<u32>,
    /// Immediate dominator; the virtual root dominates itself.
    pub idom: Vec<u32>,
    /// Bytes released with this node: its dominator subtree.
    pub retained: Vec<u64>,
    /// Shortest-path parent and the offset of its pointer word.
    pub parent: Vec<u32>,
    pub parent_offset: Vec<u32>,
    pub flags: Vec<u8>,
}

impl Graph {
    /// Virtual root index.
    pub fn root(&self) -> u32 {
        self.blocks.len() as u32
    }
    /// Node index for root region `region`.
    pub fn region(&self, region: usize) -> u32 {
        (self.blocks.len() + 1 + region) as u32
    }
    /// Unrooted node index.
    pub fn unrooted(&self) -> u32 {
        (self.blocks.len() + 1 + self.regions.len()) as u32
    }
    /// Node count including virtual nodes.
    pub fn nodes(&self) -> usize {
        self.blocks.len() + 2 + self.regions.len()
    }
    /// Outgoing edges as (target, offset).
    pub fn edges(&self, node: u32) -> impl Iterator<Item = (u32, u32)> + '_ {
        let range =
            self.edge_start[node as usize] as usize..self.edge_start[node as usize + 1] as usize;
        self.edge_to[range.clone()]
            .iter()
            .copied()
            .zip(self.edge_offset[range].iter().copied())
    }
    /// Own size of a node; virtual nodes own no bytes.
    pub fn bytes(&self, node: u32) -> u64 {
        self.blocks
            .get(node as usize)
            .map_or(0, |block| block.bytes as u64)
    }
    /// The block containing `address`.
    pub fn block_at(&self, address: usize) -> Option<u32> {
        scan::find(&self.blocks, address)
    }

    /// Scan and analyze edges, unrooted entries, shortest paths, and dominators.
    /// Sort blocks by address and discard overlapping or empty blocks.
    pub fn build(blocks: Vec<Block>, regions: Vec<Region>, memory: &impl Memory) -> Self {
        let mut graph = Self::scan(blocks, regions, memory);
        graph.analyze();
        graph
    }

    /// Scan edges alone to measure scanning separately from analysis.
    pub fn scan(blocks: Vec<Block>, regions: Vec<Region>, memory: &impl Memory) -> Self {
        scan::scan(blocks, regions, memory)
    }

    /// Analyze unrooted entries, shortest paths, dominators, and retained sizes.
    pub fn analyze(&mut self) {
        unrooted::connect(self);
        analyze::shortest_paths(self);
        analyze::dominators(self);
    }

    /// Write the binary graph consumed by `scripts/memprof_retain.py`.
    pub fn write(&self, out: &mut impl std::io::Write, header: &str) -> std::io::Result<()> {
        write::write(self, out, header)
    }

    /// Read a bounded graph, validating all indices before it can be queried.
    pub fn read(
        input: &mut impl std::io::Read,
        limits: ReadLimits,
    ) -> std::io::Result<(Self, String)> {
        read::read(input, limits)
    }
}
