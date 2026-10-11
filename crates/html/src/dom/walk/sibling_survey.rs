//! Sibling survey for walk; split out to keep the owning module within 250 lines.

use super::walk;
use crate::dom::*;
use crate::style::css::{Decls, Keyframes, Rule};
use crate::style::select::{Ancestor, Sibs, census_of};
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

/// Обойти детей узла, посчитав каждому его место среди соседей.
#[allow(clippy::too_many_arguments)]
pub(crate) fn walk_children(
    handle: &Handle,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut crate::style::generated::counters::Counters,
    path: &[Ancestor],
    preserve: bool,
    out: &mut Vec<Node>,
) {
    let children = handle.children.borrow();
    // Перепись братьев ЦЕЛИКОМ до обхода: `:nth-last-child(… of S)` смотрит
    // и на последующих, поэтому паспорта всех детей-элементов собираются
    // заранее, а каждый узел получает свою позицию в общем списке.
    let (spots, all) = census_of(&children);
    let all = Rc::new(all);
    let mut pos = 0usize;
    for (idx, (child, spot)) in children.iter().zip(&spots).enumerate() {
        let is_elem = spot.index != 0;
        let sibs = Sibs {
            all: &all,
            pos,
            is_elem,
            rc: Some(&all),
        };
        walk(
            child, rules, vars, frames, counter, counters, path, *spot, preserve, sibs, &children,
            &spots, idx, out,
        );
        pos += usize::from(is_elem);
    }
}
