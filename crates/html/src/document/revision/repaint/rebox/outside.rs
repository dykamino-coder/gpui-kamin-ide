//! What the walk around a root reads from its subtree: the box the parent's
//! walk takes (rule 2) and the counters the following siblings take (rule 3).

use super::{Walks, parent};
use crate::document::revision::mutation::Batch;
use crate::dom::Element;
use crate::dom::restyle_probe::RestyleCache;
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

/// The walk of the parent reads the same values from the root's box `a` and
/// `b` (`walk::leave`, `walk::finish_element`): its tag and level, the column
/// role a table keeps or drops, and the ruby role a ruby parent wraps by.
/// An ancestor that reads the content of its descendants (`dir=auto`, `bdi`:
/// the first strong character, HTML §3.2.6.4) reads the whole box. What the
/// passes of the build read is the spine's check.
pub(super) fn same_to_parent(a: &Element, b: &Element, root: &Handle) -> bool {
    let (x, y) = (a.style.cascaded(), b.style.cascaded());
    if x == y {
        return a.tag == b.tag && a.inline == b.inline;
    }
    let mut at = parent(root);
    let mut reads_content = false;
    while let Some(p) = at {
        if let NodeData::Element { name, attrs, .. } = &p.data {
            reads_content |= &*name.local == "bdi"
                || attrs
                    .borrow()
                    .iter()
                    .any(|a| &*a.name.local == "dir" && a.value.eq_ignore_ascii_case("auto"));
        }
        at = parent(&p);
    }
    !reads_content
        && a.tag == b.tag
        && a.inline == b.inline
        && x.reset_box_layout.display == y.reset_box_layout.display
        && x.reset_context.col_role == y.reset_context.col_role
        && x.reset_text_box.ruby_role == y.reset_text_box.ruby_role
}

/// The counters `names` the subtree of `root` leaves to its following
/// siblings are not read after it: the first following sibling the walk
/// enters instantiates each of them before and after the batch, which ends
/// their scope (css-lists-3 §4.4: a counter's scope is its creator, the
/// creator's following siblings and their descendants, up to a sibling that
/// instantiates the same name).
pub(super) fn followers_reset(
    batch: &mut Batch,
    root: &Handle,
    names: &[&str],
    (walks, values): (&Walks<'_>, &Rc<crate::values::DocumentValues>),
    cache: &mut RestyleCache,
) -> bool {
    use crate::style::computed::Display;
    use crate::style::generated::counters_scan::instantiates;
    let Some(up) = parent(root) else {
        return false;
    };
    let siblings = up.children.borrow();
    let Some(at) = siblings.iter().position(|c| Rc::ptr_eq(c, root)) else {
        return false;
    };
    let followers: Vec<Handle> = siblings[at + 1..]
        .iter()
        .filter(|c| matches!(c.data, NodeData::Element { .. }))
        .cloned()
        .collect();
    drop(siblings);
    for state in 0..2 {
        if state == 1 {
            batch.write();
        }
        let mut found = None;
        for follower in &followers {
            let one = std::slice::from_ref(follower);
            let probed = match state {
                0 => cache.kept_styles(walks.probe, walks.input, values, one),
                _ => cache.styles(walks.probe, walks.input, values, one),
            };
            let p = match probed.map(|mut v| v.pop().flatten()) {
                Ok(Some(p)) => p,
                // An element the walk does not enter takes no part in counters.
                Ok(None) => continue,
                Err(_) => break,
            };
            match p.style.reset_box_layout.display {
                Some(Display::None) => continue,
                Some(Display::Contents) => break,
                _ => {
                    found = Some(names.iter().all(|n| instantiates(&p.style, follower, n)));
                    break;
                }
            }
        }
        if state == 1 {
            batch.undo();
        }
        if found != Some(true) {
            return false;
        }
    }
    true
}
