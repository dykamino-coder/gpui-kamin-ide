//! Used styles of the cells of one table in the collapsed border model.
//!
//! A cell keeps half of its winning edges inside its box as padding and gives
//! its own border to the edge layer (CSS 2.1 §17.6.2). Cells of one computed
//! style with the same winning edges get the same used style, so the table
//! build makes it once and the cells share it, as they share the computed one.

use crate::dom::Element;
use crate::style::computed::{Computed, Sides};
use crate::style::values::value::Len;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The style the edits were made over (by address) and the values written.
type Key = (usize, [u32; 4], [Option<u32>; 4]);

/// The used styles made by this table build. An entry keeps the style it was
/// made over, so its address names no other style while the entry lives.
#[derive(Default)]
pub(in crate::layout::table) struct EdgeStyles(RefCell<HashMap<Key, (Rc<Computed>, Rc<Computed>)>>);

impl EdgeStyles {
    /// Lay the half edges `insets` and the padding `padding` (the author's
    /// plus the half edges) over the style of `cell` and drop its own border.
    pub(super) fn apply(&self, cell: &mut Element, insets: [f32; 4], padding: Sides) {
        let Some(base) = cell.style.layer() else {
            write(cell.style.used_mut(), insets, padding);
            return;
        };
        let px = |side: Option<Len>| match side {
            Some(Len::Px(v)) => Some(v.to_bits()),
            _ => None,
        };
        let key = (
            Rc::as_ptr(&base) as usize,
            insets.map(f32::to_bits),
            [padding.top, padding.right, padding.bottom, padding.left].map(px),
        );
        let mut made = self.0.borrow_mut();
        let (_, used) = made.entry(key).or_insert_with(|| {
            let mut used = base.clone();
            write(Rc::make_mut(&mut used), insets, padding);
            (base, used)
        });
        cell.style.share_layer(used.clone());
    }
}

fn write(style: &mut Computed, insets: [f32; 4], padding: Sides) {
    style.reset_background.bg_border_insets = Some(insets);
    style.reset_box_layout.padding = padding;
    style.reset_border.border_width = Default::default();
    style.reset_border.border_visible = [None; 4];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::test_support::find;

    /// A cell whose computed style is materialized, as in a document.
    fn materialized_cell() -> Element {
        let parsed = crate::document::parsed::parse("<table><tr><td>a</td></tr></table>", "");
        crate::style::cascade::materialize::materialize(
            &parsed.nodes,
            &Rc::new(Computed::default()),
            parsed.values(),
        );
        find(&parsed.nodes, &|e| e.tag == "td").unwrap().clone()
    }
    fn pad(v: f32) -> Sides {
        Sides {
            top: Some(Len::Px(v)),
            right: Some(Len::Px(v)),
            bottom: Some(Len::Px(v)),
            left: Some(Len::Px(v)),
        }
    }

    #[test]
    fn cells_of_one_style_and_edges_share_the_used_style() {
        let styles = EdgeStyles::default();
        let mut cells = [materialized_cell()];
        let mut copy = cells[0].clone();
        styles.apply(&mut cells[0], [0.5; 4], pad(1.5));
        styles.apply(&mut copy, [0.5; 4], pad(1.5));
        let (a, b) = (cells[0].style.used_rc(), copy.style.used_rc());
        assert!(Rc::ptr_eq(a.as_ref().unwrap(), b.as_ref().unwrap()));
        let used = cells[0].style.used();
        assert_eq!(used.reset_background.bg_border_insets, Some([0.5; 4]));
        assert_eq!(used.reset_box_layout.padding, pad(1.5));
        assert_eq!(styles.0.borrow().len(), 1);
    }

    #[test]
    fn other_edges_or_a_later_edit_do_not_touch_the_shared_style() {
        let styles = EdgeStyles::default();
        let mut cells = [materialized_cell()];
        let mut wide = cells[0].clone();
        let mut edited = cells[0].clone();
        styles.apply(&mut cells[0], [0.5; 4], pad(1.5));
        styles.apply(&mut wide, [1.0; 4], pad(2.0));
        styles.apply(&mut edited, [0.5; 4], pad(1.5));
        edited.style.used_mut().reset_box_layout.width = Some(Len::Px(7.0));
        assert_eq!(wide.style.used().reset_box_layout.padding, pad(2.0));
        assert_eq!(cells[0].style.used().reset_box_layout.padding, pad(1.5));
        assert_eq!(cells[0].style.used().reset_box_layout.width, None);
        assert_eq!(styles.0.borrow().len(), 2);
    }
}
