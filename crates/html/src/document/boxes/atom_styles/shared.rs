//! What the atoms of a box tree share: the parent box style of the latest
//! ones and their distinct paint results.

use super::super::BoxTree;
use crate::style::computed::BoxStyle;
use std::cell::RefCell;
use std::rc::Rc;

/// The most distinct paint results the box tree keeps for atoms to share.
const PAINTS: usize = 16;

/// Give `style` an equal paint result kept by an earlier atom, else keep its
/// own for the next ones: atoms of a few styles in many paragraphs hold one
/// result per style, not one each. Most recent first, so a run of like atoms
/// finds its result at once.
pub(super) fn share_paint(boxes: &RefCell<BoxTree>, style: &mut BoxStyle) {
    let Some(paint) = &style.paint else {
        return;
    };
    let Ok(mut boxes) = boxes.try_borrow_mut() else {
        return;
    };
    let paints = &mut boxes.atom_paints;
    match paints
        .iter()
        .position(|kept| Rc::ptr_eq(kept, paint) || **kept == **paint)
    {
        Some(at) => {
            let kept = paints.remove(at).unwrap_or_else(|| paint.clone());
            style.paint = Some(kept.clone());
            paints.push_front(kept);
        }
        None => {
            paints.truncate(PAINTS - 1);
            paints.push_front(paint.clone());
        }
    }
}

/// The kept parent if `parent` equals it, else a new one kept for the next
/// atoms: the atoms of a paragraph compare their parent once.
pub(super) fn recent_parent(boxes: &RefCell<BoxTree>, parent: &BoxStyle) -> Rc<BoxStyle> {
    let Ok(mut boxes) = boxes.try_borrow_mut() else {
        return Rc::new(parent.clone());
    };
    match &boxes.atom_parent {
        Some(kept) if same_parent(kept, parent) => kept.clone(),
        _ => {
            let kept = Rc::new(parent.clone());
            boxes.atom_parent = Some(kept.clone());
            kept
        }
    }
}

/// Every value of a parent box style that `box_style` reads.
pub(super) fn same_parent(a: &BoxStyle, b: &BoxStyle) -> bool {
    let paint = match (&a.paint, &b.paint) {
        (None, None) => true,
        (Some(x), Some(y)) => Rc::ptr_eq(x, y) || **x == **y,
        _ => false,
    };
    a.same_box_values(b)
        && paint
        && a.self_node == b.self_node
        && a.decor_box == b.decor_box
        && a.cb_node == b.cb_node
        && a.fixed_cb_node == b.fixed_cb_node
        && a.anchor_key == b.anchor_key
        && same_element(a.element_style.as_ref(), b.element_style.as_ref())
}

fn same_element(
    a: Option<&crate::style::computed::ElementStyle>,
    b: Option<&crate::style::computed::ElementStyle>,
) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            Rc::ptr_eq(&x.original, &y.original)
                && Rc::ptr_eq(&x.computed, &y.computed)
                && match (&x.parent, &y.parent) {
                    (None, None) => true,
                    (Some(p), Some(q)) => Rc::ptr_eq(p, q) || same_element(Some(p), Some(q)),
                    _ => false,
                }
        }
        _ => false,
    }
}
