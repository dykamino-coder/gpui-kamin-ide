//! Распорка роста перед перенесённой коробкой (`grow_before`): поле над ней в копии.

use crate::dom::{Element, Node};
use crate::style::values::value::Len;

/// Распорка роста: `margin-top += grow` у потомка `id`. Поле ложится в
/// `lead` меры (`shape_full`) и в раскладку копии одинаково; недобор от
/// схлопывания с большим нижним полем соседа добирает следующий заход
/// `grow_pushed`.
pub(super) fn grow_before(c: &mut Element, id: u64, grow: f32) -> bool {
    for n in c.children.iter_mut() {
        let Node::Element(k) = n else {
            continue;
        };
        if k.node_id == id {
            let old = crate::style::values::value::px_lossy_zero(k.style.used().margin.top);
            k.style.used_mut().margin.top = Some(Len::Px(old + grow));
            return true;
        }
        if grow_before(k, id, grow) {
            return true;
        }
    }
    false
}
