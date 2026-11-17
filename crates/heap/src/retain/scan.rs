//! Conservative scanning: binary-search sorted live-block intervals for every
//! aligned word in each block and root region. Interior pointers count too
//! (hashbrown tables can point inside an allocation). Deduplicate edges from
//! the same source to the same block, retaining the first pointer offset.

use super::{Block, Graph, Memory, NONE, Region};

/// The block containing `address`; blocks must be sorted and non-overlapping.
pub(super) fn find(blocks: &[Block], address: usize) -> Option<u32> {
    let index = blocks.partition_point(|block| block.address <= address);
    let block = blocks.get(index.checked_sub(1)?)?;
    (address < block.address + block.bytes).then_some((index - 1) as u32)
}

/// Block intervals with separate start and end arrays for cache locality.
struct Intervals {
    starts: Vec<usize>,
    ends: Vec<usize>,
    low: usize,
    high: usize,
}

impl Intervals {
    fn new(blocks: &[Block]) -> Self {
        let starts: Vec<usize> = blocks.iter().map(|block| block.address).collect();
        let ends: Vec<usize> = blocks
            .iter()
            .map(|block| block.address + block.bytes)
            .collect();
        Self {
            low: starts.first().copied().unwrap_or(0),
            high: ends.last().copied().unwrap_or(0),
            starts,
            ends,
        }
    }
    #[inline]
    fn find(&self, word: usize) -> Option<usize> {
        if word < self.low || word >= self.high {
            return None;
        }
        let index = self.starts.partition_point(|start| *start <= word) - 1;
        (word < self.ends[index]).then_some(index)
    }
}

/// Edges from `words` belonging to node `source`.
fn edges_of(
    graph: &mut Graph,
    intervals: &Intervals,
    mark: &mut [u32],
    source: u32,
    words: &[usize],
) {
    for (index, word) in words.iter().enumerate() {
        let Some(target) = intervals.find(*word) else {
            continue;
        };
        if target as u32 == source || mark[target] == source {
            continue;
        }
        mark[target] = source;
        graph.edge_to.push(target as u32);
        graph
            .edge_offset
            .push((index * size_of::<usize>()).min(u32::MAX as usize) as u32);
    }
}

pub(super) fn scan(mut blocks: Vec<Block>, regions: Vec<Region>, memory: &impl Memory) -> Graph {
    blocks.retain(|block| block.bytes > 0);
    blocks.sort_unstable_by_key(|block| block.address);
    // An overlap indicates stale live-block tracking; retain the first block.
    blocks.dedup_by(|next, kept| next.address < kept.address + kept.bytes);
    let intervals = Intervals::new(&blocks);
    let count = blocks.len();
    let mut graph = Graph {
        blocks,
        regions,
        ..Graph::default()
    };
    let nodes = graph.nodes();
    graph.edge_start.reserve(nodes + 1);
    let mut mark = vec![NONE; count];
    for source in 0..count {
        graph.edge_start.push(graph.edge_to.len() as u32);
        let block = graph.blocks[source];
        edges_of(
            &mut graph,
            &intervals,
            &mut mark,
            source as u32,
            memory.words(block.address, block.bytes),
        );
    }
    // The virtual root owns root regions and the unrooted node.
    graph.edge_start.push(graph.edge_to.len() as u32);
    for region in 0..graph.regions.len() {
        graph.edge_to.push(graph.region(region));
        graph.edge_offset.push(0);
    }
    graph.edge_to.push(graph.unrooted());
    graph.edge_offset.push(0);
    for region in 0..graph.regions.len() {
        graph.edge_start.push(graph.edge_to.len() as u32);
        let (start, bytes) = (graph.regions[region].start, graph.regions[region].bytes);
        let source = graph.region(region);
        edges_of(
            &mut graph,
            &intervals,
            &mut mark,
            source,
            memory.words(start, bytes),
        );
    }
    // Analysis appends edges to the final, unrooted node.
    graph.edge_start.push(graph.edge_to.len() as u32);
    graph.edge_start.push(graph.edge_to.len() as u32);
    debug_assert_eq!(graph.edge_start.len(), nodes + 1);
    graph
}
