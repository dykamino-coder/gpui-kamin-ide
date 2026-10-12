//! Build the shared decoration layer before children, including atomic boxes.

use crate::dom::Element;
use crate::paint::gap_rules::{GapGuard, fresh_gap_items, painter::GapRulePainter};
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement};

pub(crate) fn decorated_children(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    children: impl FnOnce() -> Vec<AnyElement>,
) -> Vec<AnyElement> {
    let spec = super::gap_rule_spec(e, merged, opts);
    let context = spec
        .as_ref()
        .map(|_| fresh_gap_items(e.node_id ^ opts.doc_salt));
    let _guard = context.as_ref().map(|(key, _)| GapGuard::enter(*key));
    let mut layers = Vec::new();
    if let (Some((_, buffer)), Some(spec)) = (context, spec) {
        layers.push(GapRulePainter::new(buffer, spec).into_any_element());
    }
    layers.extend(children());
    layers
}
