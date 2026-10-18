//! ::slotted aliases assigned elements; the slot prefix and argument use different trees.

use super::{Ancestor, FlatCtx, Scope};
use crate::dom::tree_index::{NodeKey, TreeIndex};
use crate::style::css::{Rule, Selector};
use crate::style::select::{Sibs, matching::matches};
use std::rc::Rc;

struct SlottedRule {
    rule: Rule,
    argument: Selector,
}

/// Originating slot context in its own tree, captured before rendering.
pub(super) struct SlotContext {
    pub(super) scope: Rc<Scope>,
    pub(super) node: Ancestor,
    pub(super) path: Vec<Ancestor>,
    pub(super) peers: Rc<Vec<Ancestor>>,
    pub(super) pos: usize,
}

/// Cache the originating slot match in the shadow scope. The argument is
/// deliberately retained for evaluation on each flattened assigned element.
fn candidates(scope: &Scope, slot: &Ancestor, path: &[Ancestor], sibs: Sibs) -> Vec<SlottedRule> {
    scope
        .rules
        .iter()
        .filter_map(|rule| {
            let arg = rule
                .sel
                .pseudo
                .as_deref()?
                .strip_prefix("slotted(")?
                .strip_suffix(')')?;
            let argument = Selector::parse(arg)?;
            if argument.ancestor.is_some()
                || argument.prev.is_some()
                || argument.has_pseudo_element()
            {
                return None;
            }
            let mut origin = rule.sel.clone();
            origin.pseudo = origin.also.pop();
            matches(&origin, slot, path, sibs).then(|| SlottedRule {
                rule: rule.clone(),
                argument,
            })
        })
        .collect()
}

/// Fallback children are not assigned to their own slot. A fallback expanded
/// through a reprojected slot is included by `flatten_slot`, as required by DOM.
pub(super) fn register_all(tree: &mut TreeIndex) {
    let slots: Vec<_> = tree.slots.values().cloned().collect();
    for slot in slots {
        if slot.assigned.is_empty() {
            continue;
        }
        let Some(origin) = &slot.origin else { continue };
        let mut slot_node = origin.node.clone();
        slot_node.slot = Some(slot.clone());
        let sibs = Sibs {
            all: &origin.peers,
            pos: origin.pos,
            is_elem: true,
            rc: Some(&origin.peers),
        };
        let rules = candidates(&origin.scope, &slot_node, &origin.path, sibs);
        for FlatCtx {
            node,
            anc,
            path,
            all,
            pos,
        } in slot.flattened.iter().flatten()
        {
            let sibs = Sibs {
                all,
                pos: *pos,
                is_elem: true,
                rc: Some(all),
            };
            let found: Vec<Rule> = rules
                .iter()
                .filter(|r| matches(&r.argument, anc, path, sibs))
                .map(|r| r.rule.clone())
                .collect();
            if !found.is_empty() {
                let entry = tree.slotted.entry(NodeKey::of(node)).or_default();
                Rc::make_mut(entry).extend(found);
            }
        }
    }
}
