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

pub(super) fn hoist_grid_abspos(
    admission: &crate::budget::admission::Admission,
    nodes: &mut [Node],
) -> Result<(), crate::budget::admission::Error> {
    crate::dom::traversal::postorder_try(admission, nodes, |el| {
        if !is_grid(el.style.cascaded()) || !own_containing_block(el.style.cascaded()) {
            return Ok(());
        }
        let mut taken = vec![];
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if own_containing_block(child.style.cascaded()) || is_grid(child.style.cascaded()) {
                continue;
            }
            steal_placed(admission, child, &mut taken)?;
        }
        el.children.extend(taken);
        Ok(())
    })
}

/// Забрать из поддерева `owner` абсолютные элементы с заданными линиями сетки. Переносится
/// коробка, а не элемент: родителем-элементом для наследования остаётся `owner`
/// (`Marks::style_parent`, css-display-3 §1).
fn steal_placed(
    admission: &crate::budget::admission::Admission,
    owner: &mut Element,
    out: &mut Vec<Node>,
) -> Result<(), crate::budget::admission::Error> {
    struct Frame {
        remaining: std::vec::IntoIter<Node>,
        kept: Vec<Node>,
        owner: Option<Element>,
        id: u64,
        lent: bool,
    }
    let mut stack = crate::budget::admission::Buffer::default();
    stack.push(
        Frame {
            remaining: std::mem::take(&mut owner.children).into_iter(),
            kept: Vec::new(),
            owner: None,
            id: owner.node_id,
            lent: false,
        },
        admission,
    )?;
    while let Some(frame) = stack.last_mut() {
        if let Some(mut node) = frame.remaining.next() {
            admission.work(1)?;
            if let Node::Element(el) = &mut node {
                let placed = el.style.cascaded().grid_col.is_some()
                    || el.style.cascaded().grid_row.is_some();
                if el.style.cascaded().position == Some(Position::Absolute) && placed {
                    if frame.id != 0 {
                        el.marks.style_parent = Some(frame.id);
                        frame.lent = true;
                    }
                    out.push(node);
                    continue;
                }
                // Свой CB и подсетка замыкают область переноса (css-grid-2 §9).
                if !own_containing_block(el.style.cascaded()) && !el.style.cascaded().subgrid {
                    let Node::Element(mut el) = node else {
                        unreachable!()
                    };
                    let remaining = std::mem::take(&mut el.children).into_iter();
                    let id = el.node_id;
                    stack.push(
                        Frame {
                            remaining,
                            kept: Vec::new(),
                            owner: Some(el),
                            id,
                            lent: false,
                        },
                        admission,
                    )?;
                    continue;
                }
            }
            frame.kept.push(node);
        } else {
            let frame = stack.pop().expect("active hoist frame");
            if let Some(mut el) = frame.owner {
                el.children = frame.kept.into();
                el.marks.lends_style |= frame.lent;
                stack
                    .last_mut()
                    .expect("hoist parent")
                    .kept
                    .push(Node::Element(el));
            } else {
                owner.children = frame.kept.into();
                owner.marks.lends_style |= frame.lent;
            }
        }
    }
    Ok(())
}

/// Контейнер сетки — это и `inline-grid`: разница только в том, как коробка
/// встаёт в поток снаружи.
fn is_grid(c: &Computed) -> bool {
    matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid))
}
