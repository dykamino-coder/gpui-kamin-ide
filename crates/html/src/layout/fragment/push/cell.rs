//! Распорка роста перед перенесённой коробкой (`grow_before`): поле над ней в копии.

use crate::dom::{Element, Node};
use crate::style::values::value::Len;

/// Распорка роста: `margin-top += grow` у потомка `id` в разложенной копии
/// (`Regrow`); недобор от схлопывания с большим нижним полем соседа добирает
/// следующий заход роста.
pub(super) fn grow_before(c: &mut Element, id: u64, grow: f32) -> bool {
    for n in c.children.iter_mut() {
        let Node::Element(k) = n else {
            continue;
        };
        if k.node_id == id {
            let old = crate::style::values::value::px_lossy_zero(
                k.style.used().reset_box_layout.margin.top,
            );
            k.style.used_mut().reset_box_layout.margin.top = Some(Len::Px(old + grow));
            return true;
        }
        if grow_before(k, id, grow) {
            return true;
        }
    }
    false
}
