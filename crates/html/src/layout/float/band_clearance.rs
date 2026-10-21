//! Клиренс в измеряемом хосте полос: какие коробки его считают и как до него доходит поле.
//! `supported` — коробка с `clear` годится в хост, если не позиционирована (позиционированной
//! нужен исходный адаптер содержащего блока); `mark_start` помечает хост, чьё верхнее поле
//! примыкает к открытому верху содержащего блока; `remember_float_margin` переносит из
//! струны схлопывания отложенное поле перед флоатом (CSS 2.1 §9.5.1, правило 5) — флоат
//! начинается после полей предшествующих блоков, даже если они физически не выложены.

use crate::dom::{Element, Node};
use crate::layout::block::struts::{Strut, solve};
use crate::render::{is_blank, out_of_flow};

pub(super) fn mark_start(host: &mut Element, top_open: bool, preceding: &[Node]) {
    if top_open
        && preceding
            .iter()
            .all(|n| is_blank(n) || matches!(n, Node::Element(e) if out_of_flow(&e.style)))
    {
        host.marks.adjoining_start = true;
    }
}

pub(super) fn supported(e: &Element) -> bool {
    // Positioned boxes need their original containing-block adapter; the
    // independently measured host cannot preserve that coordinate system.
    e.style.position.is_none()
}

pub(crate) fn remember_float_margin(e: &mut Element, strut: Option<Strut>, emitted: f32) {
    if e.style.float.is_some_and(|f| f != 0) && e.style.float_margin_offset.is_none() {
        // CSS 2.1 §9.5.1 rule 5: the float starts at the source position after
        // preceding block margins. Self-collapsing boxes can leave that
        // margin pending rather than physically emitted in the flow.
        e.style.float_margin_offset = Some(strut.map_or(0.0, |s| solve(s) - emitted));
    }
}
