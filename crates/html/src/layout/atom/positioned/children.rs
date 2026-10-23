//! Слой CB абсолютного атома: потомки разрешают размеры от его padding box.

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::layer_scope::LayerScope;
use crate::style::computed::BoxStyle;
use gpui::AnyElement;

pub(super) fn children(e: &Element, own: &BoxStyle, opts: &RenderOpts) -> Vec<AnyElement> {
    let layer = LayerScope::strict(
        &opts.cx.position.state,
        own,
        crate::style::computed::stacking::fixed_cb_layer_box(own),
    );
    let mut children = opts.cx.contracts.build.blocks(&e.children, own, opts);
    children.extend(layer.finish());
    children
}
