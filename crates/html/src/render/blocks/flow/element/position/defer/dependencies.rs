//! Зависимости отложенного OOF сохраняют идентичность исходного элемента.
use super::*;

pub(super) fn of(
    e: &Element,
    inherited: &BoxStyle,
    base_opts: &RenderOpts,
    opts: &RenderOpts,
) -> crate::layout::positioned::anchor::Dependencies {
    let vcx = opts.cx.vcx();
    let mut style = box_style(inherited, &e.style, vcx);
    style.self_node = e.node_id;
    style.anchor_key = key_of(e);
    style.anchor_seq = base_opts.cx.anchor_seq_for(e);
    crate::layout::positioned::anchor::Dependencies::of(&style, inherited, vcx, opts.cx.layout())
}
