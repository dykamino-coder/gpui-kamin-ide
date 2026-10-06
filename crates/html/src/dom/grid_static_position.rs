//! Use grid content edges for static positions and grid lines only for its own CB.
use super::Node;
use crate::computed::{Display, Position};
use crate::value::Len;

pub(super) fn adjust(nodes: &mut [Node]) {
    walk(nodes, false);
}

fn walk(nodes: &mut [Node], containing_grid: bool) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        let grid = matches!(el.style.display, Some(Display::Grid | Display::InlineGrid));
        let establishes_cb = crate::inline::establishes_cb(&el.style);
        walk(
            &mut el.children,
            if establishes_cb {
                grid
            } else {
                containing_grid
            },
        );
        if !matches!(
            el.style.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        ) {
            continue;
        }
        let static_grid = grid && !establishes_cb && !containing_grid;
        let pad = el.style.padding;
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if child.style.position != Some(Position::Absolute) {
                continue;
            }
            // CSS Grid 2 sections abspos-items/static-position: placement lines
            // apply only to a grid containing block. Retain them when an outer
            // grid is that block; this pass must not discard its placement.
            if static_grid {
                child.style.grid_col = None;
                child.style.grid_row = None;
            }
            // Элемент с заданными линиями стоит не на статической позиции, а в
            // СВОЕЙ области сетки — поля туда добавлять нечего.
            if child.style.grid_col.is_some() || child.style.grid_row.is_some() {
                continue;
            }
            // `left: auto` — это ОТСУТСТВИЕ края, а не заданный край: именно
            // при `auto` с обеих сторон элемент стоит на статической позиции.
            let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
            let inset = child.style.inset;
            if auto(inset.left) && auto(inset.right) {
                child.style.margin.left = add_len(child.style.margin.left, pad.left);
                child.style.margin.right = add_len(child.style.margin.right, pad.right);
            }
            if auto(inset.top) && auto(inset.bottom) {
                child.style.margin.top = add_len(child.style.margin.top, pad.top);
                child.style.margin.bottom = add_len(child.style.margin.bottom, pad.bottom);
            }
        }
    }
}

fn add_len(a: Option<Len>, b: Option<Len>) -> Option<Len> {
    match (a, b) {
        (Some(Len::Px(x)), Some(Len::Px(y))) => Some(Len::Px(x + y)),
        (None, b) => b,
        (a, _) => a,
    }
}
