//! Binary graph format: `kamin retain graph v1`, a u32 JSON header length and
//! header text, then little-endian values. Counts `blocks nodes edges regions`
//! are u64. Each region has u64 start/bytes and a u32-length UTF-8 name. Block
//! addresses/sizes are u64; `edge_start` has nodes+1 u32 entries; `edge_to`,
//! `edge_offset`, `idom`, `parent`, and `parent_offset` are u32; `retained` is u64
//! and `flags` is u8. The reader is `scripts/memprof_retain.py`.

use super::Graph;
use std::io::{Result, Write};

pub(super) const MAGIC: &[u8] = b"kamin retain graph v1\n";

fn u32s(out: &mut impl Write, values: &[u32]) -> Result<()> {
    for chunk in values.chunks(8192) {
        let bytes: Vec<u8> = chunk.iter().flat_map(|value| value.to_le_bytes()).collect();
        out.write_all(&bytes)?;
    }
    Ok(())
}

fn u64s(out: &mut impl Write, values: impl Iterator<Item = u64>) -> Result<()> {
    let mut bytes = Vec::with_capacity(8 * 8192);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
        if bytes.len() >= 8 * 8192 {
            out.write_all(&bytes)?;
            bytes.clear();
        }
    }
    out.write_all(&bytes)
}

pub(super) fn write(graph: &Graph, out: &mut impl Write, header: &str) -> Result<()> {
    out.write_all(MAGIC)?;
    out.write_all(&(header.len() as u32).to_le_bytes())?;
    out.write_all(header.as_bytes())?;
    let counts = [
        graph.blocks.len(),
        graph.nodes(),
        graph.edge_to.len(),
        graph.regions.len(),
    ];
    u64s(out, counts.iter().map(|count| *count as u64))?;
    for region in &graph.regions {
        u64s(out, [region.start as u64, region.bytes as u64].into_iter())?;
        out.write_all(&(region.name.len() as u32).to_le_bytes())?;
        out.write_all(region.name.as_bytes())?;
    }
    u64s(out, graph.blocks.iter().map(|block| block.address as u64))?;
    u64s(out, graph.blocks.iter().map(|block| block.bytes as u64))?;
    u32s(out, &graph.edge_start)?;
    u32s(out, &graph.edge_to)?;
    u32s(out, &graph.edge_offset)?;
    u32s(out, &graph.idom)?;
    u64s(out, graph.retained.iter().copied())?;
    u32s(out, &graph.parent)?;
    u32s(out, &graph.parent_offset)?;
    out.write_all(&graph.flags)
}
