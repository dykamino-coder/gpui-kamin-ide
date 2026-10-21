//! Слой линеек промежутков (css-gaps-1) перед детьми контейнера: при заданном стиле линеек
//! создаётся свежий буфер проб (`GapItems`) и художник `GapRulePainter`, а дети строятся с
//! настройками, где `LayoutCx::gap_items` указывает на этот буфер, чтобы их пробы, включая
//! атомарные коробки, попадали в него. Художник идёт первым слоем, под детьми.

use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::paint::gap_rules::{GapItems, painter::GapRulePainter};
use crate::style::computed::Computed;
use gpui::AnyElement;

pub(crate) fn decorated_children(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    children: impl FnOnce(&RenderOpts) -> Vec<AnyElement>,
) -> Vec<AnyElement> {
    let spec = super::gap_rule_spec(e, merged, opts);
    let context = spec.as_ref().map(|_| GapItems::default());
    let child_opts = opts.inner(|cx| {
        if let Some(items) = &context {
            cx.gap_items = Some(items.clone());
        }
    });
    let mut layers = Vec::new();
    if let (Some(buffer), Some(spec)) = (context, spec) {
        layers.push(GapRulePainter::new(buffer, spec, opts.cx.paint.clone()).hooked());
    }
    layers.extend(children(&child_opts));
    layers
}
