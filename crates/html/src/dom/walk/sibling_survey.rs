//! Обход детей узла с переписью братьев: вход в рекурсию `walk`.
//!
//! `walk_children` снимает перепись всех детей целиком до обхода
//! (`census_of`): `:nth-last-child(… of S)` смотрит и на последующих, поэтому
//! паспорта детей-элементов собираются заранее, а каждый узел получает своё
//! место (`Spot`) и список братьев (`Sibs`) при входе в `walk`.

use super::build_dom_node;
use crate::dom::tree_index::TreeIndex;
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
    tree: &TreeIndex,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut crate::style::generated::counters::Counters,
    path: &mut Vec<Ancestor>,
    preserve: bool,
    out: &mut Vec<Node>,
    vcx: crate::document::state::ValueCx<'_>,
) {
    let children = handle.children.borrow();
    // Перепись братьев ЦЕЛИКОМ до обхода: `:nth-last-child(… of S)` смотрит
    // и на последующих, поэтому паспорта всех детей-элементов собираются
    // заранее, а каждый узел получает свою позицию в общем списке.
    let (spots, all) = census_of(&children, tree);
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
        build_dom_node(
            child, tree, rules, vars, frames, counter, counters, path, *spot, preserve, sibs,
            &children, &spots, idx, out, vcx,
        );
        pos += usize::from(is_elem);
    }
}
