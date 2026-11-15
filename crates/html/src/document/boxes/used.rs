//! Used styles owned by the box tree: one shared value per (computed style, role).
//!
//! Layout derives a box's used style from its computed one (a spacer of a line
//! atom, the cell of a collapsed-border table). Writing through `BoxStyle` or
//! `SharedStyle::used_mut` copies the whole `Computed` for every box and every
//! frame. Boxes of one computed style (style sharing) and one role get the same
//! used style, so the tree keeps one value per pair, as Gecko keeps the
//! inheriting anonymous-box styles on their parent style.
//!
//! Keys are the address of the computed value, kept alive by the entry itself
//! (no address reuse while the entry exists). Entries whose computed value no
//! one else holds any more are dropped when the table is full.

use crate::style::computed::Computed;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// What a used style is derived for. Each role is a pure function of the
/// computed style it is derived from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum UsedRole {
    /// The spacer text run that holds an atomic inline's place in its line:
    /// no word or letter spacing (`render::paragraph::route::collect`).
    AtomSpacer,
    /// A table cell as the table places it (`layout::table::rows::cell_box`).
    Cell(CellUsed),
    /// A block that sets the orthogonal-flow limit of its descendants, as
    /// `f32` bits (`render::element::orthogonal`).
    OrthoLimit(u32),
}

/// The edits the table makes over a cell's style. They depend on the cell's
/// computed style and its place in the grid, so cells of one style in like
/// places share one used style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct CellUsed {
    /// A spanning cell clipped to its row group shows no scroll overflow.
    pub(crate) clear_overflow: bool,
    /// Every column of the cell is collapsed.
    pub(crate) in_collapsed_columns: bool,
    /// The spanning minimum width in px, as `f32` bits.
    pub(crate) spanning_min: Option<u32>,
    /// The column resolves the authored width, not the box.
    pub(crate) clear_width: bool,
}

const CAPACITY: usize = 4096;

struct Entry {
    base: Rc<Computed>,
    used: Rc<Computed>,
}

#[derive(Default)]
pub(crate) struct UsedStyles(RefCell<HashMap<(usize, UsedRole), Entry>>);

impl UsedStyles {
    /// The used style of `role` already derived over `base`, if any.
    pub(crate) fn find(&self, base: &Rc<Computed>, role: UsedRole) -> Option<Rc<Computed>> {
        let key = (Rc::as_ptr(base) as usize, role);
        self.0.borrow().get(&key).map(|entry| entry.used.clone())
    }

    /// Keep `used`, derived for `role` over `base` by its owner (layout or
    /// render writes it once), for the next box of the same style and role.
    pub(crate) fn insert(&self, base: &Rc<Computed>, role: UsedRole, used: Rc<Computed>) {
        let mut table = self.0.borrow_mut();
        if table.len() >= CAPACITY {
            table.retain(|_, entry| Rc::strong_count(&entry.base) > 1);
            if table.len() >= CAPACITY {
                table.clear();
            }
        }
        let base = base.clone();
        table.insert((Rc::as_ptr(&base) as usize, role), Entry { base, used });
    }
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.0.borrow().len()
    }

    #[cfg(feature = "heap-report")]
    pub(crate) fn heap_size(&self) -> usize {
        // The table and one header per used value; their groups are shared
        // with the computed styles (written groups are the few the role sets).
        let table = self.0.borrow();
        table.capacity() * size_of::<((usize, UsedRole), Entry)>()
            + table.len() * size_of::<Computed>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    fn spacer(table: &UsedStyles, base: &Rc<Computed>) -> Rc<Computed> {
        table.find(base, UsedRole::AtomSpacer).unwrap_or_else(|| {
            let mut used = (**base).clone();
            used.inh.text_flow.letter_spacing = Some(Len::Px(0.0));
            let used = Rc::new(used);
            table.insert(base, UsedRole::AtomSpacer, used.clone());
            used
        })
    }

    #[test]
    fn one_value_per_computed_style_and_role() {
        let table = UsedStyles::default();
        let base = Rc::new(Computed::default());
        let a = spacer(&table, &base);
        let b = table.find(&base, UsedRole::AtomSpacer).unwrap();
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(a.inh.text_flow.letter_spacing, Some(Len::Px(0.0)));
        assert_eq!(base.inh.text_flow.letter_spacing, None);
        assert!(table.find(&base, UsedRole::OrthoLimit(0)).is_none());
        let other = Rc::new(Computed::default());
        let c = spacer(&table, &other);
        assert!(!Rc::ptr_eq(&a, &c));
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn a_full_table_drops_only_styles_no_box_holds() {
        let table = UsedStyles::default();
        let kept = Rc::new(Computed::default());
        let first = spacer(&table, &kept);
        for _ in 1..CAPACITY {
            spacer(&table, &Rc::new(Computed::default()));
        }
        spacer(&table, &Rc::new(Computed::default()));
        assert_eq!(table.len(), 2, "only the held style and the new one stay");
        let again = table.find(&kept, UsedRole::AtomSpacer).unwrap();
        assert!(Rc::ptr_eq(&first, &again));
    }
}
