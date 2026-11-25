//! Counters of an isolated walk of one subtree: the walk starts with no
//! counter of the document and records what it reads from the counters
//! outside the subtree and what it hands to them.
//!
//! A rebuild of one subtree's boxes (`dom::walk::subtree`) does not know the
//! counters and the quote depth at its root: they are the state of the
//! document walk in tree order before it (css-lists-3 §4, css-content-3
//! §4.2). The subtree is independent of them when nothing in it reads a
//! counter it did not create or a quote; the document after it is unchanged
//! when what it hands out (updates of outside counters, the counters it
//! leaves to its following siblings) is the same before and after an edit.
//! Blink likewise renumbers list items and counters from the nearest
//! ancestor that scopes them (`CountersAttachmentContext`,
//! `ListItemOrdinal::ItemInsertedOrRemoved`), not the document.

use super::Counters;

/// What an isolated walk exchanged with the counters outside its subtree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outside {
    /// The subtree read a value it did not create: a counter of an
    /// ancestor or an earlier sibling, the quote depth or `quotes`.
    pub reads: bool,
    /// Updates of counters the subtree did not create, in walk order.
    pub hands: Vec<(String, i64, bool)>,
    /// The counters it leaves to its following siblings with their values.
    pub left: Vec<(String, i64, bool)>,
}

impl Outside {
    /// The names of the counters the subtree leaves to its following
    /// siblings that differ between `self` and `other`.
    pub fn left_changes<'a>(&'a self, other: &'a Self) -> Vec<&'a str> {
        let mut names: Vec<&str> = self
            .left
            .iter()
            .filter(|e| !other.left.contains(e))
            .chain(other.left.iter().filter(|e| !self.left.contains(e)))
            .map(|(name, _, _)| name.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

impl Counters {
    /// The counters of an isolated walk of one subtree.
    pub fn isolated() -> Self {
        Self {
            outside: Some(Box::default()),
            ..Self::default()
        }
    }

    /// The end of an isolated walk: what it exchanged with the outside;
    /// `None` for the counters of a document walk.
    pub fn outside(mut self) -> Option<Outside> {
        let mut outside = *self.outside.take()?;
        // The scope of a counter its creator at the top of the subtree made
        // covers the creator's following siblings (css-lists-3 §4.4).
        let mut left: Vec<(String, i64, bool)> = self
            .stack
            .iter()
            .flat_map(|(name, st)| {
                st.iter()
                    .filter(|e| e.owner.len() <= 1)
                    .map(move |e| (name.clone(), e.value, e.reversed))
            })
            .collect();
        left.sort();
        outside.left = left;
        Some(outside)
    }

    /// A read of the counter `name`: outside the subtree when it holds none
    /// of its own.
    pub(super) fn read(&mut self, name: &str) {
        let Some(outside) = self.outside.as_deref_mut() else {
            return;
        };
        let path = &self.path;
        let own = self.stack.get(name).is_some_and(|st| {
            st.iter()
                .rev()
                .find(|e| super::covers(super::parent(&e.owner), path))
                .is_some_and(|e| !e.outside)
        });
        outside.reads |= !own;
    }

    /// A read of a value only the outside holds.
    pub(super) fn read_outside(&mut self) {
        if let Some(outside) = self.outside.as_deref_mut() {
            outside.reads = true;
        }
    }

    /// An update of a counter the subtree did not create.
    pub(super) fn hand_out(&mut self, name: &str, delta: i64, is_set: bool) {
        if let Some(outside) = self.outside.as_deref_mut() {
            outside.hands.push((name.to_string(), delta, is_set));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subtree_that_creates_its_counters_reads_nothing_outside() {
        let mut c = Counters::isolated();
        c.enter();
        c.reset("n", 0);
        c.enter();
        c.update("n", 1, false);
        assert_eq!(c.value_of("n"), 1);
        c.leave();
        c.leave();
        let outside = c.outside().unwrap();
        assert!(!outside.reads);
        assert!(outside.hands.is_empty());
        assert_eq!(outside.left, vec![("n".to_string(), 1, false)]);
    }

    #[test]
    fn reading_or_updating_an_outer_counter_is_recorded() {
        let mut c = Counters::isolated();
        c.enter();
        c.update("list-item", 1, false);
        c.enter();
        let _ = c.value_of("list-item");
        c.leave();
        c.leave();
        let outside = c.outside().unwrap();
        assert!(outside.reads);
        assert_eq!(outside.hands, vec![("list-item".to_string(), 1, false)]);
        assert_eq!(outside.left, vec![("list-item".to_string(), 1, false)]);
        assert_eq!(Counters::default().outside(), None);
    }
}
