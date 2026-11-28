//! Bounded reader for the existing graph wire format. Counts are checked
//! against both work limits and remaining bytes before allocating arrays.

use super::{Block, Graph, NONE, Region, write::MAGIC};
use std::io::{self, Error, ErrorKind, Read};

#[derive(Clone, Copy)]
pub struct ReadLimits {
    pub file_bytes: usize,
    pub nodes: usize,
    pub edges: usize,
}

impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            file_bytes: 256 * 1024 * 1024,
            nodes: 1_048_576,
            edges: 16_777_216,
        }
    }
}

fn invalid() -> Error {
    Error::new(
        ErrorKind::InvalidData,
        "invalid or oversized retention graph",
    )
}

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, length: usize) -> io::Result<&'a [u8]> {
        if length > self.0.len() {
            return Err(invalid());
        }
        let (value, rest) = self.0.split_at(length);
        self.0 = rest;
        Ok(value)
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn count(&mut self, max: usize) -> io::Result<usize> {
        let value = usize::try_from(self.u64()?).map_err(|_| invalid())?;
        if value > max {
            Err(invalid())
        } else {
            Ok(value)
        }
    }
    fn text(&mut self, max: usize) -> io::Result<String> {
        let length = self.u32()? as usize;
        if length > max {
            return Err(invalid());
        }
        String::from_utf8(self.take(length)?.to_vec()).map_err(|_| invalid())
    }
    fn u32s(&mut self, count: usize) -> io::Result<Vec<u32>> {
        let bytes = self.take(count.checked_mul(4).ok_or_else(invalid)?)?;
        Ok(bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
            .collect())
    }
    fn u64s(&mut self, count: usize) -> io::Result<Vec<u64>> {
        let bytes = self.take(count.checked_mul(8).ok_or_else(invalid)?)?;
        Ok(bytes
            .chunks_exact(8)
            .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
            .collect())
    }
}

pub(super) fn read(input: &mut impl Read, limits: ReadLimits) -> io::Result<(Graph, String)> {
    let mut data = Vec::new();
    input
        .take(limits.file_bytes.saturating_add(1) as u64)
        .read_to_end(&mut data)?;
    if data.len() > limits.file_bytes {
        return Err(invalid());
    }
    let mut cursor = Cursor(&data);
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(invalid());
    }
    let header = cursor.text(16 * 1024 * 1024)?;
    let blocks = cursor.count(limits.nodes)?;
    let nodes = cursor.count(limits.nodes)?;
    let edges = cursor.count(limits.edges)?;
    let regions = cursor.count(1024)?;
    if blocks.checked_add(2 + regions) != Some(nodes) || nodes >= NONE as usize {
        return Err(invalid());
    }
    let mut graph = Graph::default();
    for _ in 0..regions {
        let start = usize::try_from(cursor.u64()?).map_err(|_| invalid())?;
        let bytes = usize::try_from(cursor.u64()?).map_err(|_| invalid())?;
        let name = cursor.text(4096)?;
        if start.checked_add(bytes).is_none() {
            return Err(invalid());
        }
        graph.regions.push(Region { start, bytes, name });
    }
    let addresses = cursor.u64s(blocks)?;
    let sizes = cursor.u64s(blocks)?;
    for (address, bytes) in addresses.into_iter().zip(sizes) {
        let address = usize::try_from(address).map_err(|_| invalid())?;
        let bytes = usize::try_from(bytes).map_err(|_| invalid())?;
        if bytes == 0
            || address.checked_add(bytes).is_none()
            || graph
                .blocks
                .last()
                .is_some_and(|previous| previous.address + previous.bytes > address)
        {
            return Err(invalid());
        }
        graph.blocks.push(Block { address, bytes });
    }
    graph.edge_start = cursor.u32s(nodes + 1)?;
    graph.edge_to = cursor.u32s(edges)?;
    graph.edge_offset = cursor.u32s(edges)?;
    graph.idom = cursor.u32s(nodes)?;
    graph.retained = cursor.u64s(nodes)?;
    graph.parent = cursor.u32s(nodes)?;
    graph.parent_offset = cursor.u32s(nodes)?;
    graph.flags = cursor.take(nodes)?.to_vec();
    if !cursor.0.is_empty()
        || graph.edge_start.first() != Some(&0)
        || graph.edge_start.last().copied().map(|value| value as usize) != Some(edges)
        || graph.edge_start.windows(2).any(|pair| pair[0] > pair[1])
        || graph.edge_to.iter().any(|&node| node as usize >= nodes)
        || graph
            .idom
            .iter()
            .chain(&graph.parent)
            .any(|&node| node != NONE && node as usize >= nodes)
        || graph.flags.iter().any(|flags| flags & !3 != 0)
    {
        return Err(invalid());
    }
    for source in 0..nodes as u32 {
        if source == graph.root() {
            if !graph
                .edges(source)
                .map(|(node, _)| node)
                .eq(graph.root() + 1..nodes as u32)
            {
                return Err(invalid());
            }
        } else if graph.edges(source).any(|(node, _)| node as usize >= blocks) {
            return Err(invalid());
        }
    }
    // Recompute derived data from validated edges, rather than trusting saved
    // dominator/parent cycles or fabricated retained byte totals.
    let scanned_edges = graph.edge_start[graph.unrooted() as usize] as usize;
    graph.edge_to.truncate(scanned_edges);
    graph.edge_offset.truncate(scanned_edges);
    *graph.edge_start.last_mut().unwrap() = scanned_edges as u32;
    graph.analyze();
    Ok((graph, header))
}

#[cfg(test)]
mod tests;
