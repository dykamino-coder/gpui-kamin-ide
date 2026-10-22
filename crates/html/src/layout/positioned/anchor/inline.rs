//! Рамки строчных коробок в реестре якорей: строчный якорь и строчный содержащий блок.

use super::probe::{Entry, record};
use crate::document::state::ValueCx;
use gpui::{Bounds, Pixels, Window};

/// Строчный содержащий блок `node` (CSS 2.1 §10.1 п.4: край отбивки от
/// начала первого фрагмента до конца последнего) — в реестр `CB`, как
/// блочный содержащий блок у пробы (`probe_for`): по нему считают сетку
/// `position-area` и приемлемость якоря (§target).
pub fn register_inline_cb(node: u64, own_cb: u64, padding_box: Bounds<Pixels>, vcx: ValueCx<'_>) {
    vcx.doc
        .anchor_frame
        .cb
        .borrow_mut()
        .insert(node, padding_box);
    vcx.doc
        .anchor_frame
        .cb_parent
        .borrow_mut()
        .insert(node, own_cb);
}

/// Строчный именованный якорь: рамка — объемлющий прямоугольник border
/// box его фрагментов (§anchor box), запись — та же, что у блочной пробы.
pub fn register_inline_anchor(
    names: &[String],
    who: Entry,
    border_box: Bounds<Pixels>,
    window: &mut Window,
    vcx: ValueCx<'_>,
) {
    record(names, false, who, border_box, window, vcx);
}

/// Якорный шаг сборки для абсолюта, который строится строчным путём
/// (статическая позиция в абзаце или атом): те же ключи и размеры, что у
/// `render::element` — свой `node_id`, порядок сборки, ключ клетки,
/// выбранный вариант `position-try` и `anchor-size()`.
pub fn prepare_inline_abs(
    merged: &mut crate::style::computed::Computed,
    e: &crate::dom::Element,
    inherited: &crate::style::computed::Computed,
    cx: &crate::layout::cx::LayoutCx,
) {
    let vcx = cx.vcx();
    merged.self_node = e.node_id;
    merged.anchor_seq = cx.anchor_seq_for(e);
    merged.anchor_key = super::key_of(e);
    super::resolve::apply_chosen(merged, vcx);
    super::resolve::resolve_sizes(merged, inherited, vcx);
}
