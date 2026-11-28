//! Opt-in engine profiling service. Platform counters, allocation sessions and
//! snapshot folding live below HTML and GPUI; applications only supply roots.
//! Logical GPU resource bytes, DXGI usage and process commit remain distinct.

#[cfg(feature = "allocations")]
pub mod allocation;
#[cfg(feature = "allocations")]
pub mod graph;
pub mod journal;
pub mod process;
#[cfg(feature = "allocations")]
pub mod report;
#[cfg(feature = "allocations")]
pub mod retention;
#[cfg(feature = "allocations")]
pub mod session;
#[cfg(feature = "allocations")]
pub mod symbols;
#[cfg(feature = "allocations")]
pub mod wire;
