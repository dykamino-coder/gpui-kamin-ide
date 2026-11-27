//! Who copies shared style: the source location of every write that detached a
//! shared style group (count and bytes), and the running style copy totals a
//! host attaches to its own attribution (element types of a layout pass).

use super::{Counter, Phase, Relaxed, Session, Totals, current};
use std::{collections::HashMap, panic::Location};

/// Detached style groups by the location of the write.
#[derive(Default)]
pub(super) struct Sites(HashMap<&'static Location<'static>, (u64, u64)>);

/// One copy of a shared style group of `bytes` at `site`.
pub fn clone_site(site: &'static Location<'static>, bytes: u64) {
    let _ = current(|totals| {
        if let Ok(mut sites) = totals.sites.lock() {
            let entry = sites.0.entry(site).or_default();
            entry.0 += 1;
            entry.1 += bytes;
        }
    });
}

/// The running total of `counter` over every phase of this thread's session.
pub fn running(counter: Counter) -> u64 {
    current(|totals| sum(totals, counter)).unwrap_or(0)
}

fn sum(totals: &Totals, counter: Counter) -> u64 {
    (0..Phase::COUNT)
        .map(|phase| totals.counts[phase][counter as usize].load(Relaxed))
        .sum()
}

/// Style group copies, their bytes, computed copies and their bytes so far.
pub fn style_copy_costs() -> [u64; 4] {
    current(|totals| {
        [
            Counter::StyleGroupClones,
            Counter::StyleGroupCloneBytes,
            Counter::ComputedClones,
            Counter::ComputedCloneBytes,
        ]
        .map(|counter| sum(totals, counter))
    })
    .unwrap_or_default()
}

impl Session {
    /// Detached style groups since the last take: `file:line`, count, bytes;
    /// the largest bytes first.
    pub fn take_clone_sites(&self) -> Vec<(String, u64, u64)> {
        let Ok(mut sites) = self.totals.sites.lock() else {
            return Vec::new();
        };
        let mut list: Vec<_> = sites
            .0
            .drain()
            .map(|(site, (count, bytes))| {
                (format!("{}:{}", site.file(), site.line()), count, bytes)
            })
            .collect();
        list.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
        list
    }
}

#[cfg(test)]
mod tests {
    use crate::style::computed::StyleGroup;
    use crate::work::{Counter, Session};

    #[test]
    fn a_detaching_write_is_counted_at_its_own_line() {
        let session = Session::begin();
        let shared = StyleGroup::new([0u8; 24]);
        let mut copy = shared.clone();
        let line = line!() + 1;
        copy[0] = 1;
        copy[1] = 2;
        let sites = session.take_clone_sites();
        assert_eq!(sites.len(), 1, "{sites:?}");
        assert!(
            sites[0].0.ends_with(&format!("sites.rs:{line}")),
            "{sites:?}"
        );
        assert_eq!((sites[0].1, sites[0].2), (1, 24));
        assert_eq!(super::running(Counter::StyleGroupClones), 1);
        assert_eq!(super::style_copy_costs(), [1, 24, 0, 0]);
        assert!(session.take_clone_sites().is_empty());
        assert_eq!((shared[0], copy[0]), (0, 1));
    }

    #[test]
    fn without_a_session_nothing_is_counted() {
        super::clone_site(std::panic::Location::caller(), 8);
        assert_eq!(super::style_copy_costs(), [0; 4]);
    }
}
