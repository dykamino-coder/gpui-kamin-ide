//! Counter state shared by cascade entry contexts without copying traversal prefixes.
use std::rc::Rc;
mod names;
mod operations;
mod scope;
mod sequence;
mod walk;
pub use scope::Scope;
use sequence::Sequence;

type Path = Sequence<u32>;
const PSEUDO_MARKER: u32 = u32::MAX - 2;
const PSEUDO_BEFORE: u32 = u32::MAX - 1;
const PSEUDO_AFTER: u32 = u32::MAX;

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    owner: Path,
    value: i64,
    reversed: bool,
}

/// Immutable snapshots share counter stacks, paths and quote scopes. Mutations
/// detach only balanced-tree paths; they never copy the entire counter context.
#[derive(Default, Clone, PartialEq)]
#[allow(clippy::type_complexity)]
pub struct Counters {
    stack: names::Names,
    boundaries: Sequence<Path>,
    path: Path,
    next: Sequence<u32>,
    quote_depth: usize,
    quote_languages: Sequence<(usize, Rc<str>)>,
    quotes: Sequence<(usize, Option<Rc<Vec<(String, String)>>>)>,
    resets: Sequence<(Path, Rc<str>)>,
}
fn prefix(a: &Path, count: usize, b: &Path) -> bool {
    b.len() >= count && a.iter().take(count).eq(b.iter().take(count))
}
fn covers(a: &Path, b: &Path) -> bool {
    prefix(a, a.len(), b)
}
fn same_parent(a: &Path, b: &Path) -> bool {
    a.len().saturating_sub(1) == b.len().saturating_sub(1)
        && prefix(a, a.len().saturating_sub(1), b)
}
fn remove_stale(st: &mut Sequence<Entry>, cur: &Path) {
    while st
        .last()
        .is_some_and(|top| !prefix(&top.owner, top.owner.len().saturating_sub(1), cur))
    {
        st.pop();
    }
}
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod tests;
