//! Used values of the cells of one table in the collapsed border model.
//!
//! A cell keeps half of its winning edges inside its box as padding and gives
//! its own border to the edge layer (CSS 2.1 §17.6.2). Cells whose computed
//! style shares its background, box and border groups get the same written
//! groups when their winning edges are the same, so the table build writes
//! them once and the cells share them, as they share the computed ones. The
//! cells of one style differ only by the identity of their box, which is not
//! in a group.

use crate::dom::Element;
use crate::style::computed::{
    Computed, ResetBackground, ResetBorder, ResetBoxLayout, Sides, StyleGroup,
};
use std::cell::RefCell;

const CAPACITY: usize = 32;

/// The groups this table build wrote, most recent `CAPACITY`. An entry keeps
/// the groups it was written over, so a match is the same groups, not equal
/// addresses.
#[derive(Default)]
pub(in crate::layout::table) struct EdgeStyles(RefCell<Vec<Entry>>, std::cell::Cell<usize>);

/// The three groups the write touches.
type Groups = (
    StyleGroup<ResetBackground>,
    StyleGroup<ResetBoxLayout>,
    StyleGroup<ResetBorder>,
);

struct Entry {
    /// The groups before the write.
    over: Groups,
    insets: [f32; 4],
    padding: Sides,
    /// The groups after the write.
    written: Groups,
}

fn groups(style: &Computed) -> Groups {
    (
        style.reset_background.clone(),
        style.reset_box_layout.clone(),
        style.reset_border.clone(),
    )
}

fn same(a: &Groups, b: &Groups) -> bool {
    a.0.ptr_eq(&b.0) && a.1.ptr_eq(&b.1) && a.2.ptr_eq(&b.2)
}

impl EdgeStyles {
    /// Lay the half edges `insets` and the padding `padding` (the author's
    /// plus the half edges) over the style of `cell` and drop its own border.
    pub(super) fn apply(&self, cell: &mut Element, insets: [f32; 4], padding: Sides) {
        let mut made = self.0.borrow_mut();
        let over = groups(&cell.style.used());
        let hit = made.iter().find(|entry| {
            entry.insets == insets && entry.padding == padding && same(&entry.over, &over)
        });
        let style = cell.style.used_mut();
        if let Some(entry) = hit {
            (
                style.reset_background,
                style.reset_box_layout,
                style.reset_border,
            ) = entry.written.clone();
            return;
        }
        style.reset_background.bg_border_insets = Some(insets);
        style.reset_box_layout.padding = padding;
        style.reset_border.border_width = Default::default();
        style.reset_border.border_visible = [None; 4];
        let entry = Entry {
            over,
            insets,
            padding,
            written: groups(style),
        };
        if made.len() < CAPACITY {
            made.push(entry);
        } else {
            made[self.1.get()] = entry;
            self.1.set((self.1.get() + 1) % CAPACITY);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::test_support::find;
    use crate::style::values::value::Len;
    use std::rc::Rc;

    /// Two cells of one computed style, as in a document.
    fn cells() -> [Element; 2] {
        let parsed = crate::document::parsed::parse(
            "<table><tr><td class=a>a</td><td class=b>b</td></tr></table>",
            "",
        );
        crate::style::cascade::materialize::materialize(
            &parsed.nodes,
            &Rc::new(Computed::default()),
            parsed.values(),
        );
        ["a", "b"].map(|class| {
            find(&parsed.nodes, &|e| e.attr("class") == Some(class))
                .unwrap()
                .clone()
        })
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
    fn cells_of_one_style_and_edges_share_the_written_groups() {
        let styles = EdgeStyles::default();
        let [mut a, mut b] = cells();
        styles.apply(&mut a, [0.5; 4], pad(1.5));
        styles.apply(&mut b, [0.5; 4], pad(1.5));
        let (a, b) = (a.style.used(), b.style.used());
        assert!(a.reset_background.ptr_eq(&b.reset_background));
        assert!(a.reset_box_layout.ptr_eq(&b.reset_box_layout));
        assert!(a.reset_border.ptr_eq(&b.reset_border));
        assert_eq!(a.reset_background.bg_border_insets, Some([0.5; 4]));
        assert_eq!(b.reset_box_layout.padding, pad(1.5));
        assert_eq!(b.reset_border.border_visible, [None; 4]);
        assert_eq!(styles.0.borrow().len(), 1);
    }

    #[test]
    fn other_edges_or_a_later_edit_do_not_touch_the_shared_groups() {
        let styles = EdgeStyles::default();
        let [mut a, mut wide] = cells();
        let mut edited = a.clone();
        styles.apply(&mut a, [0.5; 4], pad(1.5));
        styles.apply(&mut wide, [1.0; 4], pad(2.0));
        styles.apply(&mut edited, [0.5; 4], pad(1.5));
        edited.style.used_mut().reset_box_layout.width = Some(Len::Px(7.0));
        assert_eq!(wide.style.used().reset_box_layout.padding, pad(2.0));
        assert_eq!(a.style.used().reset_box_layout.padding, pad(1.5));
        assert_eq!(a.style.used().reset_box_layout.width, None);
        assert_eq!(styles.0.borrow().len(), 2);
    }
}
