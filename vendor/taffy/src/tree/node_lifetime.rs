//! Node teardown releases measurement contexts alongside native tree state.
//!
//! The behaviour itself now lives upstream in `TaffyTree::clear`/`TaffyTree::remove`
//! (DioxusLabs/taffy#1181); only the KaminIDE regression tests remain here.

#[cfg(test)]
#[path = "node_lifetime_tests.rs"]
mod tests;
