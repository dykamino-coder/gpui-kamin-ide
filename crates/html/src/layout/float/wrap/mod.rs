//! Обтекание флоатов: `wrap_floats` и хвост потока.
// owner: A

use std::ops::ControlFlow;

use crate::dom::Node;
use crate::style::computed::Computed;
mod prepare;
use prepare::{prepare_float_nodes, unfloated_flow};
mod covered;
mod float_run;
mod lone;
mod row;
mod run;
mod shaped;
use float_run::wrap_float_run;

pub(crate) fn wrap_floats(
    nodes: Vec<Node>,
    parent: &Computed,
    // Открыт ли ВЕРХНИЙ край содержащего блока для схлопывания с полем
    // первого ребёнка (§8.3.1). Только при открытом крае верхнее поле
    // очищающей коробки увозит вниз сам содержащий блок, а вместе с ним —
    // ПРИМЫКАЮЩИЙ флоат (Blink `block_layout_algorithm.cc:1796`).
    cb_top_open: bool,
    // Кегль содержащего блока в точках: по нему `covered_flow_tail` решает
    // `em` у флоата и соседа без своего `font-size`.
    em: f32,
    // Можно ли звать измеряемый бандовый хост (`band_host_m`): блочный
    // контейнер горизонтального письма слева направо.
    measured_ok: bool,
    // Содержащий блок — корень БФК (§10.6.7): его авто-высота обязана
    // охватить флоаты, в том числе внутри вложенных блоков.
    parent_bfc: bool,
    // The parent is a table cell (see `CELL_BFC`): a BFC root whose float
    // host keeps containing floats, while the band-host choices above stay
    // those of an ordinary block (margin-collapse-121..125, 157, 158).
    cell_bfc: bool,
) -> Vec<Node> {
    let cb_width = parent.width;
    // `clear: inherit` — сторона родителя (`clear-005`: `clear: left` на
    // контейнере и `inherit` на ребёнке). Разрешается здесь: своего
    // наследования у ненаследуемого свойства нет, а родительский стиль есть
    // только у вызывающего.
    let mut nodes = prepare_float_nodes(nodes, parent, cb_top_open);
    if let Some(value) = unfloated_flow(cb_top_open, em, measured_ok, parent_bfc, &mut nodes) {
        return value;
    }
    let mut out: Vec<Node> = vec![];
    let mut i = 0usize;
    while i < nodes.len() {
        let Node::Element(e) = &nodes[i] else {
            out.push(nodes[i].clone());
            i += 1;
            continue;
        };
        let Some(side) = e.style.float.filter(|f| *f != 0) else {
            out.push(nodes[i].clone());
            i += 1;
            continue;
        };
        if let ControlFlow::Break(_) = wrap_float_run(
            parent,
            cb_top_open,
            em,
            measured_ok,
            parent_bfc,
            cell_bfc,
            cb_width,
            &mut nodes,
            &mut out,
            &mut i,
            side,
        ) {
            continue;
        }
    }
    out
}
