//! Чистый план среза по измеренным строкам и цепочке блочных предков.

mod automatic;
mod boxes;
mod counted;
mod fit;
mod geometry;
mod paragraph;
#[cfg(test)]
mod tests;

use super::ClampEntry;
pub(crate) use boxes::ClampBox;
use boxes::{chain_end, chain_extra, chain_sizes};
pub(crate) use geometry::ClampGeometry;
