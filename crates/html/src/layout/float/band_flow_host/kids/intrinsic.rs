//! Вход постоянного intrinsic-слота: стиль узла, наследование и ресурсы документа.

use crate::dom::{Element, IntrinsicInput, IntrinsicSizes};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;

pub(super) fn sizes(
    node: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    em: f32,
    vertical: bool,
) -> IntrinsicSizes {
    let mut styles = vec![node.style.used().style, inherited.style_rc().clone()];
    styles.extend(node.first_line.iter().map(|s| s.used().style));
    styles.extend(node.first_letter.iter().map(|s| s.used().style));
    let vcx = opts.cx.vcx();
    node.children.intrinsic_sizes(IntrinsicInput {
        styles,
        viewport: vcx.viewport,
        containing: (opts.cx.constraints.cb_width, opts.cx.constraints.avail_h),
        em,
        fonts: vcx.doc.font_generation(),
        images: vcx.doc.images.borrow().snapshot().revision(),
        vertical,
    })
}
