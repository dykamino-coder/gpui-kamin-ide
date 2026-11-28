//! One graph file carries labels, epoch and capture coverage. Readers never
//! infer full-heap coverage from the observed allocation subset.

use super::{Label, View};
use crate::retention::{Capture, Coverage};
use kamin_heap::retain::{Graph, ReadLimits};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

#[derive(Serialize, Deserialize)]
pub struct Metadata {
    pub schema: u32,
    pub session: u64,
    pub epoch: u64,
    pub sampling_mean_bytes: usize,
    pub scope: String,
    pub coverage: Coverage,
    pub labels: Vec<Label>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn write(
    capture: &Capture,
    labels: Vec<Label>,
    mean: usize,
    out: &mut impl Write,
) -> io::Result<()> {
    let _suppress = crate::allocation::Suppress::enter();
    View::new(&capture.graph, &labels).map_err(invalid)?;
    let meta = Metadata {
        schema: 1,
        session: capture.session,
        epoch: capture.epoch,
        sampling_mean_bytes: mean,
        scope: "conservative graph of observed allocations; unrooted is not proof of a leak".into(),
        coverage: capture.coverage.clone(),
        labels,
    };
    let header = serde_json::to_string(&meta).map_err(io::Error::other)?;
    if header.len() > 16 * 1024 * 1024 {
        return Err(invalid("graph metadata exceeds byte limit"));
    }
    capture.graph.write(out, &header)
}

pub fn read(input: &mut impl Read) -> io::Result<(Graph, Metadata)> {
    let (graph, header) = Graph::read(input, ReadLimits::default())?;
    let meta: Metadata = serde_json::from_str(&header).map_err(io::Error::other)?;
    if meta.schema != 1 || meta.labels.len() != graph.blocks.len() {
        return Err(invalid("graph metadata schema or labels mismatch"));
    }
    Ok((graph, meta))
}
