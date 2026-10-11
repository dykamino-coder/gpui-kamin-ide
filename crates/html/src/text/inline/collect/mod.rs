//! Сборка кусков абзаца из узлов (collect_with_empty_metrics) и сдвиги наложений.

mod element_nodes;
use crate::text::inline::collect::element_nodes::collect_element;

mod boundary_spacing;
mod overlays;
mod text_nodes;
use crate::text::inline::collect::boundary_spacing::set_boundary_spacing;
pub(super) use crate::text::inline::collect::overlays::overlay_in_row;
use crate::text::inline::collect::text_nodes::collect_text;

use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use crate::text::inline::*;

pub(super) fn collect_with_empty_metrics(
    children: &[Node],
    inherited: &Computed,
    atom: &mut dyn FnMut(&Element) -> Option<Piece>,
    has_text: bool,
    case: &mut text_case::Context,
) -> Vec<Piece> {
    let mut out = vec![];
    // Место последней распорки зазора за коробкой (см. ниже).
    let mut gap_at: Option<usize> = None;
    for child in children {
        // Куски ЭТОГО ребёнка: по ним ставится зазор на границе с тем, что
        // идёт следом (см. `set_boundary_spacing` ниже).
        let from = out.len();
        match child {
            Node::Text(t) => {
                collect_text(t, inherited, case, &mut out);
            }
            Node::Element(e) => {
                collect_element(
                    e,
                    inherited,
                    atom,
                    has_text,
                    case,
                    &mut out,
                    &mut gap_at,
                    from,
                );
            }
        }
    }
    // За последним куском границы внутри ЭТОГО узла нет: зазор там задаст тот
    // предок, у которого дальше идёт своё содержимое. Он и поставит его на
    // этот же кусок, когда сборка вернётся к нему.
    set_boundary_spacing(&mut out, None);
    if gap_at.is_some_and(|at| at + 1 == out.len()) {
        out.pop();
    }
    out
}
