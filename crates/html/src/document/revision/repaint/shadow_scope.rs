//! The light tree's side of an edit in a document with shadow trees
//! (`ShadowFacts`): the probe cascades the light tree with the document's
//! rules only, which is the whole cascade of an element outside every host's
//! subtree (css-scoping-1 §3.1: a shadow tree's rules reach only its own
//! elements, its host and the light children assigned to its slots). The
//! edits the probe cannot decide that way restyle the document:
//!
//! - a name the batch changes that a shadow sheet mentions (`:host()`,
//!   `:host-context()`, `::slotted()` and the shadow's own selectors may read
//!   it), or a shadow sheet whose text is unknown (`<link>`);
//! - an element to restyle that is a host or lies in a host's light subtree
//!   (its cascade takes the host's `:host` rules, or it inherits through a
//!   slot of the flat tree);
//! - a subtree to rebuild that holds a host (its boxes take a shadow tree).
//!
//! Stylo restyles each shadow root's scope with its own `CascadeData`; here
//! a light edit that no shadow scope can see keeps the restyle of its own
//! elements, and the rest takes a rebuild.

use super::super::mentions::mentions;
use crate::dom::invalidation::Changed;
use crate::dom::restyle_probe::RestyleProbe;
use markup5ever_rcdom::Handle;
use std::rc::Rc;

/// Why the probe cannot restyle `elements` for the batch of names `changed`,
/// if it cannot.
pub(super) fn refused(
    probe: &RestyleProbe,
    changed: &Changed,
    elements: &[Handle],
) -> Option<&'static str> {
    let shadows = probe.shadows();
    if shadows.hosts.is_empty() {
        return None;
    }
    let read = |css: &String| mentions(css, &changed.attrs, &changed.classes);
    if shadows.linked || shadows.sheets.iter().any(read) {
        return Some("shadow_reads");
    }
    let hosted = |e: &Handle| shadows.hosts.iter().any(|h| within(e, h));
    elements.iter().any(hosted).then_some("shadow_host")
}

/// Whether a subtree of `roots` holds a host.
pub(super) fn holds_host(probe: &RestyleProbe, roots: &[Handle]) -> bool {
    let hosts = &probe.shadows().hosts;
    hosts.iter().any(|h| roots.iter().any(|r| within(h, r)))
}

/// `node` is `ancestor` or lies under it.
fn within(node: &Handle, ancestor: &Handle) -> bool {
    let mut at = Some(node.clone());
    while let Some(n) = at {
        if Rc::ptr_eq(&n, ancestor) {
            return true;
        }
        let weak = n.parent.take();
        at = weak.as_ref().and_then(|w| w.upgrade());
        n.parent.set(weak);
    }
    false
}
