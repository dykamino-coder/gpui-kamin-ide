//! Слой CB абсолютного атома: потомки разрешают размеры от его padding box.

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::layer_scope::LayerScope;
use crate::style::computed::Computed;
use gpui::AnyElement;

pub(super) fn children(e: &Element, own: &Computed, opts: &RenderOpts) -> Vec<AnyElement> {
    let layer = LayerScope::strict(
        &opts.cx.positioning,
        own,
        crate::paint::stacking::fixed_cb_layer_box(own),
    );
    let mut children = opts.cx.build.blocks(&e.children, own, opts);
    children.extend(layer.finish());
    children
}
