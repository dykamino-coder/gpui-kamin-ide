//! The `:has()` marks of a probe after a batch that changes a name some
//! `:has()` arguments read: only those arguments are marked again, the marks
//! of the others hold, and only the retained entry contexts whose ancestor
//! records carry a mark that changed are dropped.
//!
//! Blink invalidates `:has()` by the features of each argument
//! (`StyleEngine::InvalidateAncestorsOrSiblingsAffectedByHas`,
//! `style_engine.cc:1676`) and re-evaluates only the anchors those features
//! reach; it keeps every other computed style. Here an argument whose
//! features the batch changes is marked again over the document, and the
//! anchors whose marks differ are the only ones whose kept records go
//! (`Chains::forget_marked`): their children enter under new records.

use super::RestyleCache;
use crate::budget::admission::Error;
use crate::dom::tree_index::{HandleKey, HasMarks, TreeIndex};
use crate::style::select::has::{HasArg, mark_has};
use markup5ever_rcdom::Handle;
use std::collections::{HashMap, HashSet};

/// Marks of the arguments `ids`, by node.
type Marked = HashMap<HandleKey, Vec<u64>>;

impl RestyleCache {
    /// Put the `:has()` marks of the probe into `tree` for the current state
    /// of the DOM under `document`: the kept marks, with those of the
    /// arguments the batch reads (`remark`) made again.
    pub(super) fn marks(
        &mut self,
        document: &Handle,
        args: &[HasArg],
        tree: &mut TreeIndex,
    ) -> Result<(), Error> {
        let affected: Vec<&HasArg> = self.remark.iter().map(|&i| &args[i]).collect();
        match self.has.take() {
            Some(has) if affected.is_empty() => tree.put_has(has),
            Some(has) => {
                let ids: Vec<u64> = affected.iter().map(|a| a.id).collect();
                let before = only(&has, &ids);
                tree.put_has(has);
                for marks in tree.has_marks.values_mut() {
                    marks.retain(|id| !ids.contains(id));
                }
                tree.has_marks.retain(|_, marks| !marks.is_empty());
                mark_has(document, &affected, &mut vec![], tree)?;
                let after = only_in(&tree.has_marks, &ids);
                self.chains.forget_marked(&changed(&before, &after));
                self.remarks += 1;
            }
            None => {
                let all: Vec<&HasArg> = args.iter().collect();
                mark_has(document, &all, &mut vec![], tree)?;
                self.remarks += 1;
            }
        }
        Ok(())
    }

    /// Whole-document markings of `:has()` arguments since the last call:
    /// each costs a pass over the document.
    pub fn take_remarks(&mut self) -> u64 {
        std::mem::take(&mut self.remarks)
    }
}

fn only(has: &HasMarks, ids: &[u64]) -> Marked {
    only_in(has.marks(), ids)
}

fn only_in(marks: &HashMap<HandleKey, Vec<u64>>, ids: &[u64]) -> Marked {
    let mut out = Marked::new();
    for (key, list) in marks {
        let mut mine: Vec<u64> = list.iter().copied().filter(|id| ids.contains(id)).collect();
        if !mine.is_empty() {
            mine.sort_unstable();
            out.insert(key.clone(), mine);
        }
    }
    out
}

/// The nodes whose marks of the compared arguments differ.
fn changed(before: &Marked, after: &Marked) -> HashSet<HandleKey> {
    let mut out: HashSet<HandleKey> = before
        .iter()
        .filter(|(key, marks)| after.get(*key) != Some(*marks))
        .map(|(key, _)| key.clone())
        .collect();
    out.extend(
        after
            .keys()
            .filter(|key| !before.contains_key(*key))
            .cloned(),
    );
    out
}
