//! Build the shared decoration layer before children, including atomic boxes.

use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::paint::gap_rules::{GapGuard, GapItems, painter::GapRulePainter};
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use gpui::AnyElement;

pub(crate) fn decorated_children(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    children: impl FnOnce() -> Vec<AnyElement>,
) -> Vec<AnyElement> {
    let spec = super::gap_rule_spec(e, merged, opts);
    let context = spec.as_ref().map(|_| GapItems::default());
    let _guard = context.clone().map(GapGuard::enter);
    let mut layers = Vec::new();
    if let (Some(buffer), Some(spec)) = (context, spec) {
        layers.push(GapRulePainter::new(buffer, spec).hooked());
    }
    layers.extend(children());
    layers
}
