//! Правки сетки: подсетка (дорожки родителя), fr в точки, лунки как сетка, абсолютные в сетке.

mod lanes;
mod subgrid_geometry;
mod subgrid_tracks;
pub(super) use crate::dom::fixup_grid::lanes::lanes_as_grid;
pub(crate) use crate::dom::fixup_grid::lanes::lanes_row_dir;
pub(crate) use crate::dom::fixup_grid::lanes::lanes_to_grid;
use crate::dom::fixup_grid::subgrid_geometry::subgrid_gap_slice;
pub(crate) use crate::dom::fixup_grid::subgrid_geometry::subgrid_inhibited;
use crate::dom::fixup_grid::subgrid_geometry::subgrid_slot;
use crate::dom::fixup_grid::subgrid_geometry::subgrid_span;
pub(super) use crate::dom::fixup_grid::subgrid_tracks::subgrid_takes_parent_tracks;

use crate::dom::*;
use crate::style::computed::{Computed, Display, Position};

pub(super) fn hoist_grid_abspos(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        hoist_grid_abspos(&mut el.children);
        if !is_grid(&el.style) || !own_containing_block(&el.style) {
            continue;
        }
        let mut taken = vec![];
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if own_containing_block(&child.style) || is_grid(&child.style) {
                continue;
            }
            steal_placed(&mut child.children, &mut taken);
        }
        el.children.extend(taken);
    }
}

/// Забрать из поддерева абсолютные элементы с заданными линиями сетки.
fn steal_placed(children: &mut Vec<Node>, out: &mut Vec<Node>) {
    let mut kept = Vec::with_capacity(children.len());
    for mut node in children.drain(..) {
        if let Node::Element(el) = &mut node {
            let placed = el.style.grid_col.is_some() || el.style.grid_row.is_some();
            if el.style.position == Some(Position::Absolute) && placed {
                out.push(node);
                continue;
            }
            // Свой содержащий блок — дальше уже чужие абсолютные элементы.
            // Останавливает и ПОДСЕТКА: она размещает своих абсолютных детей
            // сама, и счёт с конца (`grid-column: 3 / -1`) идёт по её
            // собственному числу дорожек (`subgrid/abs-pos-001`). А вот обычная
            // вложенная сетка без своего отсчёта помехой не служит: её потомок
            // по-прежнему принадлежит внешней сетке (css-grid-2 §9).
            if !own_containing_block(&el.style) && !el.style.subgrid {
                steal_placed(&mut el.children, out);
            }
        }
        kept.push(node);
    }
    *children = kept;
}

/// Контейнер сетки — это и `inline-grid`: разница только в том, как коробка
/// встаёт в поток снаружи.
fn is_grid(c: &Computed) -> bool {
    matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid))
}
