//! Слой линеек промежутков (css-gaps-1) перед детьми контейнера: при заданном стиле линеек
//! создаётся свежий буфер проб (`GapItems`) и художник `GapRulePainter`, а дети строятся с
//! настройками, где `DeferredPaintContext::gap_items` указывает на этот буфер, чтобы их пробы, включая
//! атомарные коробки, попадали в него. Художник идёт первым слоем, под детьми.

use crate::dom::Element;
use crate::layout::build_contracts::Layer;
use crate::layout::multicol::gap_rules::GapItems;
use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use gpui::AnyElement;

pub(crate) fn decorated_children(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    children: impl FnOnce(&RenderOpts) -> Vec<AnyElement>,
) -> Vec<AnyElement> {
    let (mut layers, child_opts) = prepare_children(e, merged, opts);
    layers.extend(children(&child_opts));
    layers
}

/// Контекст линеек живёт до завершения построения детей.
pub(crate) fn prepare_children(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
) -> (Vec<AnyElement>, Box<RenderOpts>) {
    let spec = super::gap_rule_spec(e, merged, opts);
    let context = spec.as_ref().map(|_| GapItems::default());
    let child_opts = opts.inner(|cx| {
        if let Some(items) = &context {
            cx.deferred.gap_items = Some(items.clone());
        }
    });
    let mut layers = Vec::new();
    if let (Some(buffer), Some(spec)) = (context, spec) {
        layers.push(
            opts.cx
                .contracts
                .build
                .layer(Layer::gap_rules(buffer, spec, &opts.cx)),
        );
    }
    (layers, child_opts)
}
